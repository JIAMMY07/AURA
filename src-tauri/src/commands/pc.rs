use super::{audit::append_audit, storage::AppState};
use serde::Serialize;
use std::{cmp::Reverse, process::Command};
use sysinfo::{Disks, System};
use tauri::State;

const MAX_PROCESSES: usize = 50;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSnapshot {
    os_name: String,
    os_version: String,
    cpu_usage_percent: f32,
    logical_cpus: usize,
    memory_used_bytes: u64,
    memory_total_bytes: u64,
    process_count: usize,
    uptime_seconds: u64,
    disks: Vec<DiskSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiskSnapshot {
    name: String,
    mount_point: String,
    available_bytes: u64,
    total_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshot {
    pid: u32,
    name: String,
    memory_bytes: u64,
    cpu_usage_percent: f32,
}

#[tauri::command]
pub fn pc_system_snapshot(state: State<'_, AppState>) -> Result<SystemSnapshot, String> {
    let snapshot = collect_system_snapshot();
    append_audit(
        &state,
        "pc-system-snapshot",
        "tool.executed",
        "allow",
        "Lettura metriche PC",
        r#"{"capability":"system.inspect"}"#,
    )?;
    Ok(snapshot)
}

fn collect_system_snapshot() -> SystemSnapshot {
    let mut system = System::new_all();
    system.refresh_all();
    let disks = Disks::new_with_refreshed_list()
        .iter()
        .map(|disk| DiskSnapshot {
            name: disk.name().to_string_lossy().chars().take(128).collect(),
            mount_point: disk
                .mount_point()
                .to_string_lossy()
                .chars()
                .take(260)
                .collect(),
            available_bytes: disk.available_space(),
            total_bytes: disk.total_space(),
        })
        .collect();
    SystemSnapshot {
        os_name: System::name().unwrap_or_else(|| "Windows".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "sconosciuta".to_string()),
        cpu_usage_percent: system.global_cpu_usage(),
        logical_cpus: system.cpus().len(),
        memory_used_bytes: system.used_memory(),
        memory_total_bytes: system.total_memory(),
        process_count: system.processes().len(),
        uptime_seconds: System::uptime(),
        disks,
    }
}

#[tauri::command]
pub fn pc_list_processes(state: State<'_, AppState>) -> Result<Vec<ProcessSnapshot>, String> {
    let mut system = System::new_all();
    system.refresh_all();
    let mut processes = system
        .processes()
        .values()
        .map(|process| ProcessSnapshot {
            pid: process.pid().as_u32(),
            name: process.name().to_string_lossy().chars().take(128).collect(),
            memory_bytes: process.memory(),
            cpu_usage_percent: process.cpu_usage(),
        })
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| Reverse(process.memory_bytes));
    processes.truncate(MAX_PROCESSES);
    append_audit(
        &state,
        "pc-process-list",
        "tool.executed",
        "allow",
        "Elenco processi limitato",
        r#"{"capability":"system.processes","limit":50}"#,
    )?;
    Ok(processes)
}

#[tauri::command]
pub fn pc_open_application(
    application: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let (executable, label) = match application.as_str() {
        "calculator" => ("calc.exe", "Calcolatrice"),
        "notepad" => ("notepad.exe", "Blocco note"),
        "explorer" => ("explorer.exe", "Esplora file"),
        _ => return Err("Applicazione non presente nell'allowlist.".to_string()),
    };
    if !confirm_native_action(
        "Conferma azione AURA",
        &format!("AURA vuole aprire {label}. Consentire questa singola azione?"),
    ) {
        append_audit(
            &state,
            "pc-open-application",
            "tool.cancelled",
            "confirm",
            &format!("Apertura annullata: {label}"),
            &serde_json::json!({ "capability": "app.open", "application": application })
                .to_string(),
        )?;
        return Ok(false);
    }
    Command::new(executable)
        .spawn()
        .map_err(|_| "Avvio applicazione Windows fallito.".to_string())?;
    append_audit(
        &state,
        "pc-open-application",
        "tool.executed",
        "confirm",
        &format!("Apertura controllata: {label}"),
        &serde_json::json!({ "capability": "app.open", "application": application }).to_string(),
    )?;
    Ok(true)
}

#[cfg(windows)]
pub(crate) fn confirm_native_action(title: &str, message: &str) -> bool {
    use std::ptr::null_mut;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONQUESTION, MB_SETFOREGROUND, MB_YESNO,
    };
    let title = title.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let message = message.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    // SAFETY: both buffers are NUL-terminated UTF-16 and live for the duration of the call.
    unsafe {
        MessageBoxW(
            null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2 | MB_SETFOREGROUND,
        ) == IDYES
    }
}

#[cfg(not(windows))]
pub(crate) fn confirm_native_action(_title: &str, _message: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::collect_system_snapshot;

    #[test]
    fn application_allowlist_is_intentionally_small() {
        assert!(matches!(
            "calculator",
            "calculator" | "notepad" | "explorer"
        ));
        assert!(!matches!(
            "powershell",
            "calculator" | "notepad" | "explorer"
        ));
    }

    #[test]
    fn real_system_snapshot_has_sane_bounds() {
        let snapshot = collect_system_snapshot();
        assert!(snapshot.logical_cpus > 0);
        assert!(snapshot.memory_total_bytes > 0);
        assert!(snapshot.memory_used_bytes <= snapshot.memory_total_bytes);
        assert!(snapshot.cpu_usage_percent >= 0.0);
    }
}
