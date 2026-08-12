use super::{audit::append_audit, policy::enforce_capability, storage::AppState};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};
use sysinfo::{Disks, Pid, System};
use tauri::State;

const MAX_PROCESSES: usize = 50;
static HAND_DESKTOP_ARMED: AtomicBool = AtomicBool::new(false);

#[derive(Deserialize)]
pub struct HandPointerMove {
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
pub struct HandScroll {
    delta: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSnapshot {
    os_name: String,
    os_version: String,
    pub(crate) cpu_usage_percent: f32,
    logical_cpus: usize,
    pub(crate) memory_used_bytes: u64,
    pub(crate) memory_total_bytes: u64,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApplication {
    id: String,
    label: String,
    source: &'static str,
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
        "display-settings" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["ms-settings:display"],
            label: "Impostazioni schermo",
        },
        "notification-settings" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["ms-settings:notifications"],
            label: "Impostazioni notifiche",
        },
        "microphone-privacy" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["ms-settings:privacy-microphone"],
            label: "Privacy microfono",
        },
        "windows-update" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["ms-settings:windowsupdate"],
            label: "Windows Update",
        },
        "downloads" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["shell:Downloads"],
            label: "Download",
        },
        "documents" => AllowedApplication {
            executable: "explorer.exe",
            arguments: &["shell:Personal"],
            label: "Documenti",
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

pub(crate) fn collect_system_snapshot() -> SystemSnapshot {
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

#[tauri::command]
pub fn pc_media_control(action: String, state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("media.control", "allow")?;
    #[cfg(windows)]
    let key = media_virtual_key(&action)
        .ok_or_else(|| "Comando multimediale non presente nell'allowlist.".to_string())?;
    #[cfg(not(windows))]
    return Err("Controlli multimediali disponibili solo su Windows.".to_string());
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};
        keybd_event(key, 0, 0, 0);
        keybd_event(key, 0, KEYEVENTF_KEYUP, 0);
    }
    append_audit(
        &state,
        "pc-media-control",
        "tool.executed",
        "allow",
        "Controllo multimediale tipizzato",
        &serde_json::json!({"capability":"media.control","action":action}).to_string(),
    )?;
    Ok(true)
}

#[tauri::command]
pub fn pc_hand_control_set(enabled: bool, state: State<'_, AppState>) -> Result<bool, String> {
    if enabled {
        enforce_capability("desktop.gesture.control", "confirm")?;
        if !confirm_native_action(
            "Abilita controllo gestuale AURA",
            "Consentire alla videocamera locale di controllare puntatore, click, trascinamento, scroll e finestra attiva? Il controllo si disattiva quando la videocamera si ferma. Il gesto di chiusura richiede una posa prolungata.",
        ) {
            return Ok(false);
        }
    }
    HAND_DESKTOP_ARMED.store(enabled, Ordering::SeqCst);
    append_audit(
        &state,
        "pc-hand-control",
        if enabled {
            "desktop.gesture.armed"
        } else {
            "desktop.gesture.disarmed"
        },
        if enabled { "confirm" } else { "allow" },
        if enabled {
            "Controllo gestuale desktop abilitato"
        } else {
            "Controllo gestuale desktop disabilitato"
        },
        r#"{"cameraFramesStored":false,"coordinatesLogged":false}"#,
    )?;
    Ok(enabled)
}

fn require_hand_desktop_armed() -> Result<(), String> {
    if HAND_DESKTOP_ARMED.load(Ordering::SeqCst) {
        Ok(())
    } else {
        Err("Controllo gestuale desktop non armato.".to_string())
    }
}

#[tauri::command]
pub fn pc_hand_pointer_move(input: HandPointerMove) -> Result<bool, String> {
    require_hand_desktop_armed()?;
    if !input.x.is_finite()
        || !input.y.is_finite()
        || !(0.0..=1.0).contains(&input.x)
        || !(0.0..=1.0).contains(&input.y)
    {
        return Err("Coordinate gestuali non valide.".to_string());
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SetCursorPos, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
            SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        };
        let origin_x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let origin_y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1);
        Ok(SetCursorPos(
            origin_x + (input.x * f64::from(width - 1)).round() as i32,
            origin_y + (input.y * f64::from(height - 1)).round() as i32,
        ) != 0)
    }
    #[cfg(not(windows))]
    Err("Controllo puntatore disponibile solo su Windows.".to_string())
}

#[tauri::command]
pub fn pc_hand_pointer_action(action: String) -> Result<bool, String> {
    require_hand_desktop_armed()?;
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            mouse_event, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_RIGHTDOWN,
            MOUSEEVENTF_RIGHTUP,
        };
        let flags = match action.as_str() {
            "left-click" => vec![MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP],
            "double-click" => vec![
                MOUSEEVENTF_LEFTDOWN,
                MOUSEEVENTF_LEFTUP,
                MOUSEEVENTF_LEFTDOWN,
                MOUSEEVENTF_LEFTUP,
            ],
            "left-down" => vec![MOUSEEVENTF_LEFTDOWN],
            "left-up" => vec![MOUSEEVENTF_LEFTUP],
            "right-click" => vec![MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP],
            _ => return Err("Azione puntatore non consentita.".to_string()),
        };
        for flag in flags {
            mouse_event(flag, 0, 0, 0, 0);
        }
        Ok(true)
    }
    #[cfg(not(windows))]
    Err("Controllo puntatore disponibile solo su Windows.".to_string())
}

#[tauri::command]
pub fn pc_hand_scroll(input: HandScroll) -> Result<bool, String> {
    require_hand_desktop_armed()?;
    if input.delta == 0 || input.delta.abs() > 1_200 {
        return Err("Delta scroll fuori limite.".to_string());
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{mouse_event, MOUSEEVENTF_WHEEL};
        mouse_event(MOUSEEVENTF_WHEEL, 0, 0, input.delta, 0);
        Ok(true)
    }
    #[cfg(not(windows))]
    Err("Scroll gestuale disponibile solo su Windows.".to_string())
}

#[tauri::command]
pub fn pc_hand_window_action(action: String, state: State<'_, AppState>) -> Result<bool, String> {
    require_hand_desktop_armed()?;
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            keybd_event, KEYEVENTF_KEYUP, VK_MENU, VK_TAB,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, IsZoomed, PostMessageW, ShowWindow, SW_MAXIMIZE, SW_MINIMIZE,
            SW_RESTORE, WM_CLOSE,
        };
        let window = GetForegroundWindow();
        if window.is_null() {
            return Err("Nessuna finestra attiva.".to_string());
        }
        match action.as_str() {
            "minimize" => {
                ShowWindow(window, SW_MINIMIZE);
            }
            "maximize-restore" => {
                ShowWindow(
                    window,
                    if IsZoomed(window) != 0 {
                        SW_RESTORE
                    } else {
                        SW_MAXIMIZE
                    },
                );
            }
            "switch-next" => {
                keybd_event(VK_MENU as u8, 0, 0, 0);
                keybd_event(VK_TAB as u8, 0, 0, 0);
                keybd_event(VK_TAB as u8, 0, KEYEVENTF_KEYUP, 0);
                keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
            }
            "close" => {
                PostMessageW(window, WM_CLOSE, 0, 0);
            }
            _ => return Err("Azione finestra non consentita.".to_string()),
        }
    }
    #[cfg(not(windows))]
    return Err("Controllo finestre disponibile solo su Windows.".to_string());
    append_audit(
        &state,
        "pc-hand-window",
        "desktop.gesture.window",
        "allow",
        "Azione gestuale sulla finestra attiva",
        &serde_json::json!({"action":action}).to_string(),
    )?;
    Ok(true)
}

#[tauri::command]
pub fn pc_window_control(action: String, state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("window.control", "confirm")?;
    if !matches!(action.as_str(), "minimize" | "maximize-restore" | "close") {
        return Err("Azione finestra non consentita.".to_string());
    }
    if action == "close"
        && !confirm_native_action(
            "Chiudi finestra attiva",
            "Chiudere la finestra attualmente in primo piano? L'applicazione potrebbe chiedere di salvare il lavoro.",
        )
    {
        return Ok(false);
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, IsZoomed, PostMessageW, ShowWindow, SW_MAXIMIZE, SW_MINIMIZE,
            SW_RESTORE, WM_CLOSE,
        };
        let window = GetForegroundWindow();
        if window.is_null() {
            return Err("Nessuna finestra attiva.".to_string());
        }
        match action.as_str() {
            "minimize" => {
                ShowWindow(window, SW_MINIMIZE);
            }
            "maximize-restore" => {
                ShowWindow(
                    window,
                    if IsZoomed(window) != 0 {
                        SW_RESTORE
                    } else {
                        SW_MAXIMIZE
                    },
                );
            }
            "close" => {
                PostMessageW(window, WM_CLOSE, 0, 0);
            }
            _ => unreachable!(),
        }
    }
    #[cfg(not(windows))]
    return Err("Controllo finestre disponibile solo su Windows.".to_string());
    append_audit(
        &state,
        "pc-window-control",
        "window.controlled",
        if action == "close" {
            "confirm"
        } else {
            "allow"
        },
        "Finestra attiva controllata tramite comando tipizzato",
        &serde_json::json!({"action":action}).to_string(),
    )?;
    Ok(true)
}

#[tauri::command]
pub fn pc_list_installed_applications(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledApplication>, String> {
    enforce_capability("app.inspect", "allow")?;
    let shortcuts = installed_shortcuts();
    let mut applications = shortcuts
        .into_iter()
        .filter_map(|path| {
            let normalized = path.to_string_lossy().to_lowercase();
            let id = state.crypto.hmac_hex(normalized.as_bytes()).ok()?;
            let label = path
                .file_stem()?
                .to_string_lossy()
                .chars()
                .take(160)
                .collect::<String>();
            Some(InstalledApplication {
                id,
                label,
                source: "Menu Start Windows",
            })
        })
        .collect::<Vec<_>>();
    applications.sort_by_key(|application| application.label.to_lowercase());
    applications.dedup_by(|left, right| left.label.eq_ignore_ascii_case(&right.label));
    applications.truncate(300);
    append_audit(
        &state,
        "pc-installed-applications",
        "tool.executed",
        "allow",
        "Catalogo applicazioni Start letto",
        &serde_json::json!({"capability":"app.inspect","count":applications.len()}).to_string(),
    )?;
    Ok(applications)
}

#[tauri::command]
pub fn pc_open_installed_application(
    application_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("app.open", "confirm")?;
    if application_id.len() != 64 || !application_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("ID applicazione non valido.".to_string());
    }
    let selected = installed_shortcuts()
        .into_iter()
        .find(|path| {
            let normalized = path.to_string_lossy().to_lowercase();
            state
                .crypto
                .hmac_hex(normalized.as_bytes())
                .map(|id| id == application_id)
                .unwrap_or(false)
        })
        .ok_or_else(|| "Applicazione non più presente nel Menu Start.".to_string())?;
    let label = selected
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("applicazione")
        .chars()
        .take(160)
        .collect::<String>();
    if !confirm_native_action(
        "Conferma avvio applicazione AURA",
        &format!("Aprire l'applicazione installata?\n\n{label}\n\nIl collegamento proviene dal Menu Start di Windows."),
    ) {
        return Ok(false);
    }
    shell_open_shortcut(&selected)?;
    append_audit(
        &state,
        "pc-open-installed-application",
        "tool.executed",
        "confirm",
        "Applicazione Start avviata",
        r#"{"capability":"app.open","source":"start-menu"}"#,
    )?;
    Ok(true)
}

fn installed_shortcuts() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(program_data) = std::env::var_os("ProgramData") {
        roots.push(
            PathBuf::from(program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Some(app_data) = std::env::var_os("APPDATA") {
        roots.push(
            PathBuf::from(app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    collect_shortcuts(&roots, 300)
}

fn collect_shortcuts(roots: &[PathBuf], limit: usize) -> Vec<PathBuf> {
    fn visit(directory: &Path, depth: usize, limit: usize, output: &mut Vec<PathBuf>) {
        if depth > 8 || output.len() >= limit {
            return;
        }
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            if output.len() >= limit {
                break;
            }
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_dir() {
                visit(&path, depth + 1, limit, output);
            } else if metadata.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("lnk"))
            {
                output.push(path);
            }
        }
    }
    let mut output = Vec::new();
    for root in roots {
        visit(root, 0, limit, &mut output);
    }
    output
}

#[cfg(windows)]
fn shell_open_shortcut(path: &Path) -> Result<(), String> {
    use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
    use windows_sys::Win32::{UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL};
    let encoded = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: the path buffer is NUL-terminated and valid for the duration of the call.
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            null_mut(),
            encoded.as_ptr(),
            null_mut(),
            null_mut(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        return Err("Avvio del collegamento Windows fallito.".to_string());
    }
    Ok(())
}

#[cfg(not(windows))]
fn shell_open_shortcut(_path: &Path) -> Result<(), String> {
    Err("Catalogo applicazioni disponibile soltanto su Windows.".to_string())
}

fn media_virtual_key(action: &str) -> Option<u8> {
    match action {
        "volume-up" => Some(0xAF),
        "volume-down" => Some(0xAE),
        "mute" => Some(0xAD),
        "play-pause" => Some(0xB3),
        "next" => Some(0xB0),
        "previous" => Some(0xB1),
        _ => None,
    }
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
    use super::{
        allowed_application, collect_shortcuts, collect_system_snapshot, media_virtual_key,
        protected_process,
    };
    use std::fs;

    #[test]
    fn application_allowlist_is_intentionally_small() {
        assert!(allowed_application("calculator").is_some());
        assert!(allowed_application("bluetooth-settings").is_some());
        assert!(allowed_application("microphone-privacy").is_some());
        assert!(allowed_application("downloads").is_some());
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
    fn media_controls_are_closed_allowlist() {
        assert_eq!(media_virtual_key("volume-up"), Some(0xAF));
        assert_eq!(media_virtual_key("play-pause"), Some(0xB3));
        assert_eq!(media_virtual_key("launch-shell"), None);
    }

    #[test]
    fn real_system_snapshot_has_sane_bounds() {
        let snapshot = collect_system_snapshot();
        assert!(snapshot.logical_cpus > 0);
        assert!(snapshot.memory_total_bytes > 0);
        assert!(snapshot.memory_used_bytes <= snapshot.memory_total_bytes);
        assert!(snapshot.cpu_usage_percent >= 0.0);
    }

    #[test]
    fn start_menu_catalog_includes_only_shortcuts() {
        let temp = tempfile::tempdir().expect("temp");
        fs::write(temp.path().join("Editor.lnk"), b"fixture").expect("lnk");
        fs::write(temp.path().join("malware.exe"), b"fixture").expect("exe");
        let shortcuts = collect_shortcuts(&[temp.path().to_path_buf()], 10);
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(
            shortcuts[0].file_name().and_then(|value| value.to_str()),
            Some("Editor.lnk")
        );
    }
}
