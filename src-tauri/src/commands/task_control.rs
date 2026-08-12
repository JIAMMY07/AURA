use super::{audit::append_audit, storage::AppState};
use serde::Serialize;
use std::{
    collections::HashSet,
    process::{Command, Stdio},
    sync::{atomic::Ordering, LazyLock, Mutex},
};
use tauri::{AppHandle, State};
use uuid::Uuid;

static ACTIVE_CHILD_PIDS: LazyLock<Mutex<HashSet<u32>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

pub(crate) struct ManagedChildRegistration(u32);

impl Drop for ManagedChildRegistration {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE_CHILD_PIDS.lock() {
            active.remove(&self.0);
        }
    }
}

pub(crate) fn register_child(pid: u32) -> ManagedChildRegistration {
    if let Ok(mut active) = ACTIVE_CHILD_PIDS.lock() {
        active.insert(pid);
    }
    ManagedChildRegistration(pid)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStopResult {
    generations_cancelled: usize,
    child_processes_signalled: usize,
    tts_stopped: bool,
    browser_closed: bool,
    message: String,
}

fn terminate_registered_children() -> usize {
    let pids = ACTIVE_CHILD_PIDS
        .lock()
        .map(|active| active.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    for pid in &pids {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let mut command = Command::new("taskkill.exe");
            command
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000);
            let _ = command.status();
        }
    }
    pids.len()
}

#[tauri::command]
pub async fn task_stop_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TaskStopResult, String> {
    let generations_cancelled = super::ollama::cancel_all_generations(&state)?;
    let tts_stopped = super::voice::stop_tts_unchecked(&state)?;
    let browser_closed = super::browser::emergency_stop(&app);
    let child_processes_signalled =
        tauri::async_runtime::spawn_blocking(terminate_registered_children)
            .await
            .map_err(|_| "Arresto dei processi gestiti interrotto.".to_string())?;
    state.voice_busy.store(false, Ordering::Release);
    append_audit(
        &state,
        &Uuid::new_v4().to_string(),
        "task.emergency_stop",
        "allow",
        "Arresto globale delle task richiesto",
        &serde_json::json!({
            "generationsCancelled": generations_cancelled,
            "childProcessesSignalled": child_processes_signalled,
            "ttsStopped": tts_stopped,
            "browserClosed": browser_closed
        })
        .to_string(),
    )?;
    Ok(TaskStopResult {
        generations_cancelled,
        child_processes_signalled,
        tts_stopped,
        browser_closed,
        message: "Task AURA arrestate; browser chiuso e controllo restituito all'utente.".into(),
    })
}
