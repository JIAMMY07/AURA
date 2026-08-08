use super::{audit::append_audit, policy::enforce_capability, storage::AppState};
use serde::Serialize;
use std::{cmp::Reverse, process::Command};
use sysinfo::{Disks, Pid, System};
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

struct AllowedApplication {
    executable: &'static str,
    arguments: &'static [&'static str],
    label: &'static str,
}

fn allowed_application(application: &str) -> Option<AllowedApplication> {
    let value = match application {
        "calculator" => AllowedApplication {
            executable: "calc.exe",
            arguments: &[],
            label: "Calcolatrice",
        },
        "notepad" => AllowedApplication {
            executable: "notepad.exe",
            arguments: &[],
            label: "Blocco note",
        },
        "explorer" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &[],
            label: "Esplora file",
        },
        "task-manager" => AllowedApplication {
            executable: "taskmgr.exe",
            arguments: &[],
            label: "Gestione attivita",
        },
        "paint" => AllowedApplication {
            executable: "mspaint.exe",
            arguments: &[],
            label: "Paint",
        },
        "snipping-tool" => AllowedApplication {
            executable: "SnippingTool.exe",
            arguments: &[],
            label: "Strumento di cattura",
        },
        "settings" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["ms-settings:"],
            label: "Impostazioni Windows",
        },
        "bluetooth-settings" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["ms-settings:bluetooth"],
            label: "Impostazioni Bluetooth",
        },
        "sound-settings" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["ms-settings:sound"],
            label: "Impostazioni audio",
        },
        "security-settings" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["windowsdefender:"],
            label: "Sicurezza di Windows",
        },
        _ => return None,
    };
    Some(value)
}

#[tauri::command]
pub fn pc_system_snapshot(state: State<'_, AppState>) -> Result<SystemSnapshot, String> {
    enforce_capability("system.inspect", "allow")?;
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
    enforce_capability("system.processes", "allow")?;
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
    enforce_capability("app.open", "confirm")?;
    let allowed = allowed_application(&application)
        .ok_or_else(|| "Applicazione non presente nell'allowlist.".to_string())?;
    let label = allowed.label;
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
    Command::new(allowed.executable)
        .args(allowed.arguments)
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

#[tauri::command]
pub fn pc_terminate_process(pid: u32, state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("process.terminate", "confirm")?;
    if pid == 0 || pid == std::process::id() {
        return Err("PID non terminabile da AURA.".to_string());
    }
    let mut system = System::new_all();
    system.refresh_all();
    let process = system
        .process(Pid::from_u32(pid))
        .ok_or_else(|| "Processo non trovato.".to_string())?;
    let name = process
        .name()
        .to_string_lossy()
        .chars()
        .take(128)
        .collect::<String>();
    if protected_process(&name) {
        return Err("Processo di sistema protetto: azione negata.".to_string());
    }
    if !confirm_native_action(
        "Conferma azione AURA",
        &format!(
            "Terminare il processo {name} (PID {pid})? I dati non salvati potrebbero andare persi."
        ),
    ) {
        append_audit(
            &state,
            "pc-terminate-process",
            "tool.cancelled",
            "confirm",
            "Terminazione processo annullata",
            &serde_json::json!({"capability":"process.terminate","pid":pid}).to_string(),
        )?;
        return Ok(false);
    }
    if !process.kill() {
        return Err("Windows non ha consentito di terminare il processo.".to_string());
    }
    append_audit(
        &state,
        "pc-terminate-process",
        "tool.executed",
        "confirm",
        "Processo terminato dopo conferma",
        &serde_json::json!({"capability":"process.terminate","pid":pid}).to_string(),
    )?;
    Ok(true)
}

fn protected_process(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "system"
            | "registry"
            | "smss.exe"
            | "csrss.exe"
            | "wininit.exe"
            | "services.exe"
            | "lsass.exe"
            | "svchost.exe"
            | "winlogon.exe"
            | "dwm.exe"
            | "explorer.exe"
            | "fontdrvhost.exe"
            | "memory compression"
            | "local-ai-assistant.exe"
    )
}

#[tauri::command]
pub fn pc_lock_workstation(state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("system.lock", "confirm")?;
    if !confirm_native_action(
        "Conferma azione AURA",
        "Bloccare ora la sessione Windows? Per rientrare servira il PIN o la password.",
    ) {
        return Ok(false);
    }
    #[cfg(windows)]
    let locked = unsafe { windows_sys::Win32::System::Shutdown::LockWorkStation() != 0 };
    #[cfg(not(windows))]
    let locked = false;
    if !locked {
        return Err("Blocco sessione Windows fallito.".to_string());
    }
    append_audit(
        &state,
        "pc-lock-workstation",
        "tool.executed",
        "confirm",
        "Sessione Windows bloccata",
        r#"{"capability":"system.lock"}"#,
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
    use super::{allowed_application, collect_system_snapshot, protected_process};

    #[test]
    fn application_allowlist_is_intentionally_small() {
        assert!(allowed_application("calculator").is_some());
        assert!(allowed_application("bluetooth-settings").is_some());
        assert!(allowed_application("powershell").is_none());
        assert!(allowed_application("cmd").is_none());
    }

    #[test]
    fn critical_windows_processes_are_protected() {
        assert!(protected_process("LSASS.EXE"));
        assert!(protected_process("explorer.exe"));
        assert!(protected_process("local-ai-assistant.exe"));
        assert!(!protected_process("synthetic-user-app.exe"));
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
