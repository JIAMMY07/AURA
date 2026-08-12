use super::{
    audit::append_audit, pc::confirm_native_action, policy::enforce_capability, storage::AppState,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::State;

const MAX_GRANTS: usize = 8;
const MAX_TEXT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 500;
const MAX_SEARCH_RESULTS: usize = 200;
const MAX_SEARCH_DEPTH: usize = 12;
const GRANTS_AAD: &[u8] = b"aura-workspace-grants-v1";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGrant {
    id: String,
    name: String,
    path: String,
    created_at: String,
}

#[derive(Deserialize, Serialize)]
struct GrantEnvelope {
    version: u8,
    ciphertext: String,
    nonce: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    relative_path: String,
    name: String,
    is_directory: bool,
    size_bytes: u64,
    modified_unix: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReadResult {
    relative_path: String,
    content: String,
    size_bytes: u64,
}

#[tauri::command]
pub fn workspace_list_grants(state: State<'_, AppState>) -> Result<Vec<WorkspaceGrant>, String> {
    enforce_capability("workspace.inspect", "allow")?;
    load_grants(&state)
}

#[tauri::command]
pub fn workspace_add_grant(
    path: String,
    state: State<'_, AppState>,
) -> Result<Option<WorkspaceGrant>, String> {
    enforce_capability("workspace.grant", "confirm")?;
    if path.trim().len() > 1024 {
        return Err("Percorso workspace troppo lungo.".to_string());
    }
    let canonical = fs::canonicalize(path.trim())
        .map_err(|_| "La cartella indicata non esiste o non è accessibile.".to_string())?;
    validate_root_scope(&canonical)?;
    if !confirm_native_action(
        "Autorizza workspace AURA",
        &format!(
            "Consentire ad AURA di leggere e, solo dopo ulteriori conferme, modificare questa cartella?\n\n{}",
            canonical.display()
        ),
    ) {
        return Ok(None);
    }
    let mut grants = load_grants(&state)?;
    if let Some(existing) = grants
        .iter()
        .find(|grant| same_path(Path::new(&grant.path), &canonical))
    {
        return Ok(Some(existing.clone()));
    }
    if grants.len() >= MAX_GRANTS {
        return Err(format!(
            "Limite di {MAX_GRANTS} workspace autorizzati raggiunto."
        ));
    }
    let name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Workspace")
        .chars()
        .take(120)
        .collect::<String>();
    let grant = WorkspaceGrant {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        path: canonical.to_string_lossy().to_string(),
        created_at: unix_seconds().to_string(),
    };
    grants.push(grant.clone());
    save_grants(&state, &grants)?;
    audit_workspace(
        &state,
        "workspace-grant",
        "workspace.grant",
        "confirm",
        "Workspace autorizzato",
    )?;
    Ok(Some(grant))
}

#[tauri::command]
pub fn workspace_remove_grant(
    grant_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("workspace.grant", "confirm")?;
    let mut grants = load_grants(&state)?;
    let Some(grant) = grants.iter().find(|grant| grant.id == grant_id) else {
        return Err("Workspace non trovato.".to_string());
    };
    if !confirm_native_action(
        "Revoca workspace AURA",
        &format!(
            "Revocare l'accesso AURA a {}? Nessun file verrà eliminato.",
            grant.name
        ),
    ) {
        return Ok(false);
    }
    grants.retain(|grant| grant.id != grant_id);
    save_grants(&state, &grants)?;
    audit_workspace(
        &state,
        "workspace-revoke",
        "workspace.grant",
        "confirm",
        "Workspace revocato",
    )?;
    Ok(true)
}

#[tauri::command]
pub fn workspace_list_entries(
    grant_id: String,
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceEntry>, String> {
    enforce_capability("workspace.read", "allow")?;
    let root = grant_root(&state, &grant_id)?;
    let directory = resolve_existing(&root, &relative_path, true)?;
    if !directory.is_dir() {
        return Err("Il percorso richiesto non è una cartella.".to_string());
    }
    let mut entries = fs::read_dir(&directory)
        .map_err(|_| "Impossibile leggere la cartella.".to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| entry_to_view(&root, &entry.path()).ok())
        .take(MAX_ENTRIES)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .is_directory
            .cmp(&left.is_directory)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    audit_workspace(
        &state,
        "workspace-list",
        "workspace.read",
        "allow",
        "Cartella workspace letta",
    )?;
    Ok(entries)
}

#[tauri::command]
pub fn workspace_search(
    grant_id: String,
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceEntry>, String> {
    enforce_capability("workspace.read", "allow")?;
    let query = query.trim().to_lowercase();
    if query.len() < 2 || query.len() > 128 {
        return Err("La ricerca deve contenere da 2 a 128 caratteri.".to_string());
    }
    let root = grant_root(&state, &grant_id)?;
    let mut results = Vec::new();
    search_directory(&root, &root, &query, 0, &mut results)?;
    audit_workspace(
        &state,
        "workspace-search",
        "workspace.read",
        "allow",
        "Ricerca workspace eseguita",
    )?;
    Ok(results)
}

#[tauri::command]
pub fn workspace_read_text(
    grant_id: String,
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<WorkspaceReadResult, String> {
    enforce_capability("workspace.read", "allow")?;
    let root = grant_root(&state, &grant_id)?;
    let target = resolve_existing(&root, &relative_path, false)?;
    let metadata = fs::metadata(&target).map_err(|_| "File non leggibile.".to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_TEXT_BYTES {
        return Err("AURA legge soltanto file di testo fino a 2 MiB.".to_string());
    }
    let bytes = fs::read(&target).map_err(|_| "Lettura file fallita.".to_string())?;
    if bytes.iter().take(8192).any(|byte| *byte == 0) {
        return Err("Il file sembra binario: lettura testuale negata.".to_string());
    }
    let content =
        String::from_utf8(bytes).map_err(|_| "Il file non è UTF-8 valido.".to_string())?;
    audit_workspace(
        &state,
        "workspace-read",
        "workspace.read",
        "allow",
        "File workspace letto",
    )?;
    Ok(WorkspaceReadResult {
        relative_path: relative_display(&root, &target)?,
        size_bytes: metadata.len(),
        content,
    })
}

#[tauri::command]
pub fn workspace_write_text(
    grant_id: String,
    relative_path: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("workspace.write", "confirm")?;
    if content.len() as u64 > MAX_TEXT_BYTES {
        return Err("Contenuto oltre il limite di 2 MiB.".to_string());
    }
    let root = grant_root(&state, &grant_id)?;
    let target = resolve_for_create(&root, &relative_path)?;
    let exists = target.exists();
    if exists && !target.is_file() {
        return Err("La destinazione non è un file.".to_string());
    }
    let action = if exists { "sovrascrivere" } else { "creare" };
    if !confirm_native_action(
        "Conferma modifica file AURA",
        &format!(
            "AURA vuole {action} il file:\n\n{}\n\nDimensione: {} byte",
            target.display(),
            content.len()
        ),
    ) {
        return Ok(false);
    }
    atomic_write(&target, content.as_bytes())?;
    audit_workspace(
        &state,
        "workspace-write",
        "workspace.write",
        "confirm",
        "File workspace modificato",
    )?;
    Ok(true)
}

#[tauri::command]
pub fn workspace_create_directory(
    grant_id: String,
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("workspace.write", "confirm")?;
    let root = grant_root(&state, &grant_id)?;
    let target = resolve_for_create(&root, &relative_path)?;
    if target.exists() {
        return Err("La destinazione esiste già.".to_string());
    }
    if !confirm_native_action(
        "Conferma cartella AURA",
        &format!("Creare la cartella?\n\n{}", target.display()),
    ) {
        return Ok(false);
    }
    fs::create_dir(&target).map_err(|_| "Creazione cartella fallita.".to_string())?;
    audit_workspace(
        &state,
        "workspace-mkdir",
        "workspace.write",
        "confirm",
        "Cartella workspace creata",
    )?;
    Ok(true)
}

#[tauri::command]
pub fn workspace_copy(
    grant_id: String,
    source: String,
    destination: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("workspace.move", "confirm")?;
    let root = grant_root(&state, &grant_id)?;
    let source = resolve_existing(&root, &source, false)?;
    if !source.is_file() {
        return Err("La copia ricorsiva di cartelle non è ancora consentita.".to_string());
    }
    let destination = resolve_for_create(&root, &destination)?;
    if destination.exists() {
        return Err("La destinazione esiste già: nessuna sovrascrittura implicita.".to_string());
    }
    if !confirm_native_action(
        "Conferma copia AURA",
        &format!(
            "Copiare il file?\n\nDa: {}\nA: {}",
            source.display(),
            destination.display()
        ),
    ) {
        return Ok(false);
    }
    fs::copy(&source, &destination).map_err(|_| "Copia file fallita.".to_string())?;
    audit_workspace(
        &state,
        "workspace-copy",
        "workspace.move",
        "confirm",
        "File workspace copiato",
    )?;
    Ok(true)
}

#[tauri::command]
pub fn workspace_move(
    grant_id: String,
    source: String,
    destination: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("workspace.move", "confirm")?;
    let root = grant_root(&state, &grant_id)?;
    let source = resolve_existing(&root, &source, false)?;
    let destination = resolve_for_create(&root, &destination)?;
    if destination.exists() {
        return Err("La destinazione esiste già: nessuna sovrascrittura implicita.".to_string());
    }
    if !confirm_native_action(
        "Conferma spostamento AURA",
        &format!(
            "Spostare l'elemento?\n\nDa: {}\nA: {}",
            source.display(),
            destination.display()
        ),
    ) {
        return Ok(false);
    }
    fs::rename(&source, &destination).map_err(|_| {
        "Spostamento fallito; origine e destinazione devono essere sullo stesso volume.".to_string()
    })?;
    audit_workspace(
        &state,
        "workspace-move",
        "workspace.move",
        "confirm",
        "Elemento workspace spostato",
    )?;
    Ok(true)
}

#[tauri::command]
pub fn workspace_trash(
    grant_id: String,
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("workspace.delete", "confirm")?;
    let root = grant_root(&state, &grant_id)?;
    let target = resolve_existing(&root, &relative_path, false)?;
    if same_path(&root, &target) {
        return Err("La radice workspace non può essere eliminata.".to_string());
    }
    if !confirm_native_action(
        "Conferma Cestino AURA",
        &format!("Spostare nel Cestino di Windows?\n\n{}\n\nL'elemento potrà essere ripristinato manualmente.", target.display()),
    ) {
        return Ok(false);
    }
    trash::delete(&target)
        .map_err(|_| "Windows non ha consentito lo spostamento nel Cestino.".to_string())?;
    audit_workspace(
        &state,
        "workspace-trash",
        "workspace.delete",
        "confirm",
        "Elemento inviato al Cestino",
    )?;
    Ok(true)
}

#[tauri::command]
pub fn workspace_open(
    grant_id: String,
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("workspace.open", "confirm")?;
    let root = grant_root(&state, &grant_id)?;
    let target = resolve_existing(&root, &relative_path, true)?;
    if target.is_file() && is_executable_like(&target) {
        return Err("Esecuzione di programmi o script dal workspace negata. Usa un runner sandbox dedicato.".to_string());
    }
    if !confirm_native_action(
        "Conferma apertura AURA",
        &format!("Aprire con Windows?\n\n{}", target.display()),
    ) {
        return Ok(false);
    }
    shell_open(&target)?;
    audit_workspace(
        &state,
        "workspace-open",
        "workspace.open",
        "confirm",
        "Elemento workspace aperto",
    )?;
    Ok(true)
}

fn grants_path(state: &AppState) -> PathBuf {
    state.app_data.join("workspace-grants.v1.json")
}

fn load_grants(state: &AppState) -> Result<Vec<WorkspaceGrant>, String> {
    let path = grants_path(state);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let encoded =
        fs::read_to_string(&path).map_err(|_| "Registro workspace non leggibile.".to_string())?;
    if encoded.len() > 128 * 1024 {
        return Err("Registro workspace fuori dai limiti.".to_string());
    }
    let envelope: GrantEnvelope =
        serde_json::from_str(&encoded).map_err(|_| "Registro workspace non valido.".to_string())?;
    if envelope.version != 1 {
        return Err("Versione registro workspace non supportata.".to_string());
    }
    let ciphertext = STANDARD
        .decode(envelope.ciphertext)
        .map_err(|_| "Registro workspace non valido.".to_string())?;
    let nonce = STANDARD
        .decode(envelope.nonce)
        .map_err(|_| "Registro workspace non valido.".to_string())?;
    let plaintext = state
        .crypto
        .decrypt_with_aad(&ciphertext, &nonce, GRANTS_AAD)?;
    let grants: Vec<WorkspaceGrant> = serde_json::from_str(&plaintext)
        .map_err(|_| "Registro workspace decifrato non valido.".to_string())?;
    if grants.len() > MAX_GRANTS {
        return Err("Registro workspace oltre il limite.".to_string());
    }
    Ok(grants)
}

fn save_grants(state: &AppState, grants: &[WorkspaceGrant]) -> Result<(), String> {
    let plaintext = serde_json::to_string(grants)
        .map_err(|_| "Serializzazione workspace fallita.".to_string())?;
    let (ciphertext, nonce) = state.crypto.encrypt_with_aad(&plaintext, GRANTS_AAD)?;
    let envelope = GrantEnvelope {
        version: 1,
        ciphertext: STANDARD.encode(ciphertext),
        nonce: STANDARD.encode(nonce),
    };
    let encoded = serde_json::to_vec(&envelope)
        .map_err(|_| "Serializzazione workspace fallita.".to_string())?;
    let target = grants_path(state);
    let temporary = state
        .app_data
        .join(format!("workspace-grants-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temporary, encoded)
        .map_err(|_| "Scrittura registro workspace fallita.".to_string())?;
    if target.exists() {
        fs::remove_file(&target)
            .map_err(|_| "Aggiornamento registro workspace fallito.".to_string())?;
    }
    fs::rename(&temporary, &target)
        .map_err(|_| "Consolidamento registro workspace fallito.".to_string())
}

fn grant_root(state: &AppState, grant_id: &str) -> Result<PathBuf, String> {
    if grant_id.len() > 64 {
        return Err("ID workspace non valido.".to_string());
    }
    let grants = load_grants(state)?;
    let grant = grants
        .iter()
        .find(|grant| grant.id == grant_id)
        .ok_or_else(|| "Workspace non autorizzato.".to_string())?;
    let root = fs::canonicalize(&grant.path)
        .map_err(|_| "La radice workspace non è più disponibile.".to_string())?;
    validate_root_scope(&root)?;
    Ok(root)
}

fn validate_root_scope(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err("Il workspace deve essere una cartella.".to_string());
    }
    if root.components().count() <= 3 {
        return Err("Radice troppo ampia: scegli una cartella di progetto specifica.".to_string());
    }
    let sensitive = [
        "WINDIR",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "APPDATA",
        "LOCALAPPDATA",
    ];
    for variable in sensitive {
        if let Some(value) = std::env::var_os(variable) {
            let sensitive_path = PathBuf::from(value);
            if path_within(root, &sensitive_path) || same_path(root, &sensitive_path) {
                return Err(
                    "Cartella di sistema o dati applicativi non autorizzabile come workspace."
                        .to_string(),
                );
            }
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        if same_path(root, Path::new(&home)) {
            return Err(
                "L'intero profilo utente è troppo ampio: scegli una sottocartella.".to_string(),
            );
        }
    }
    let sensitive_names = [".ssh", ".gnupg", "credentials", "credential", "secrets"];
    if root.components().any(|component| {
        let value = component.as_os_str().to_string_lossy().to_lowercase();
        sensitive_names.contains(&value.as_str())
    }) {
        return Err("Cartella contenente credenziali o segreti non autorizzabile.".to_string());
    }
    Ok(())
}

fn relative_path(input: &str, allow_empty: bool) -> Result<PathBuf, String> {
    if input.len() > 1024 {
        return Err("Percorso relativo troppo lungo.".to_string());
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return if allow_empty {
            Ok(PathBuf::new())
        } else {
            Err("Percorso relativo richiesto.".to_string())
        };
    }
    let mut result = PathBuf::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(value) => result.push(value),
            _ => {
                return Err(
                    "Percorso non valido: sono vietati radici, prefissi, '.' e '..'.".to_string(),
                )
            }
        }
    }
    Ok(result)
}

fn resolve_existing(root: &Path, input: &str, allow_empty: bool) -> Result<PathBuf, String> {
    let root =
        fs::canonicalize(root).map_err(|_| "Radice workspace non disponibile.".to_string())?;
    let relative = relative_path(input, allow_empty)?;
    let target = fs::canonicalize(root.join(relative))
        .map_err(|_| "Percorso workspace non trovato.".to_string())?;
    if !path_within(&target, &root) && !same_path(&target, &root) {
        return Err("Uscita dal workspace bloccata.".to_string());
    }
    Ok(target)
}

fn resolve_for_create(root: &Path, input: &str) -> Result<PathBuf, String> {
    let root =
        fs::canonicalize(root).map_err(|_| "Radice workspace non disponibile.".to_string())?;
    let relative = relative_path(input, false)?;
    let target = root.join(relative);
    let parent = target
        .parent()
        .ok_or_else(|| "Destinazione non valida.".to_string())?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|_| "La cartella padre non esiste.".to_string())?;
    if !path_within(&canonical_parent, &root) && !same_path(&canonical_parent, &root) {
        return Err("Uscita dal workspace bloccata.".to_string());
    }
    let name = target
        .file_name()
        .ok_or_else(|| "Nome destinazione non valido.".to_string())?;
    Ok(canonical_parent.join(name))
}

fn entry_to_view(root: &Path, path: &Path) -> Result<WorkspaceEntry, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "Metadati non leggibili.".to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("Collegamento simbolico escluso.".to_string());
    }
    let canonical = fs::canonicalize(path).map_err(|_| "Elemento non risolvibile.".to_string())?;
    if !path_within(&canonical, root) && !same_path(&canonical, root) {
        return Err("Junction o reparse point fuori workspace escluso.".to_string());
    }
    Ok(WorkspaceEntry {
        relative_path: relative_display(root, &canonical)?,
        name: canonical
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("?")
            .chars()
            .take(256)
            .collect(),
        is_directory: metadata.is_dir(),
        size_bytes: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        modified_unix: metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_secs())
            .unwrap_or(0),
    })
}

fn search_directory(
    root: &Path,
    directory: &Path,
    query: &str,
    depth: usize,
    results: &mut Vec<WorkspaceEntry>,
) -> Result<(), String> {
    if depth > MAX_SEARCH_DEPTH || results.len() >= MAX_SEARCH_RESULTS {
        return Ok(());
    }
    let entries = fs::read_dir(directory)
        .map_err(|_| "Ricerca interrotta da una cartella non leggibile.".to_string())?;
    for entry in entries.filter_map(Result::ok) {
        if results.len() >= MAX_SEARCH_RESULTS {
            break;
        }
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) if !value.file_type().is_symlink() => value,
            _ => continue,
        };
        let Ok(canonical) = fs::canonicalize(&path) else {
            continue;
        };
        if !path_within(&canonical, root) && !same_path(&canonical, root) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.to_lowercase().contains(query) {
            if let Ok(view) = entry_to_view(root, &canonical) {
                results.push(view);
            }
        }
        if metadata.is_dir() {
            let _ = search_directory(root, &canonical, query, depth + 1, results);
        }
    }
    Ok(())
}

fn atomic_write(target: &Path, content: &[u8]) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "Destinazione non valida.".to_string())?;
    let token = uuid::Uuid::new_v4();
    let temporary = parent.join(format!(".aura-write-{token}.tmp"));
    let backup = parent.join(format!(".aura-backup-{token}.tmp"));
    fs::write(&temporary, content).map_err(|_| "Scrittura temporanea fallita.".to_string())?;
    let existed = target.exists();
    if existed {
        fs::rename(target, &backup)
            .map_err(|_| "Impossibile preparare il rollback del file.".to_string())?;
    }
    match fs::rename(&temporary, target) {
        Ok(()) => {
            if existed {
                let _ = fs::remove_file(&backup);
            }
            Ok(())
        }
        Err(_) => {
            let _ = fs::remove_file(&temporary);
            if existed {
                let _ = fs::rename(&backup, target);
            }
            Err("Scrittura atomica fallita; il file precedente è stato ripristinato.".to_string())
        }
    }
}

fn is_executable_like(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "exe"
            | "com"
            | "bat"
            | "cmd"
            | "ps1"
            | "msi"
            | "msp"
            | "js"
            | "jse"
            | "vbs"
            | "vbe"
            | "wsf"
            | "scr"
            | "cpl"
            | "lnk"
            | "url"
    )
}

#[cfg(windows)]
fn shell_open(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
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
        return Err("Windows non ha trovato un'applicazione associata.".to_string());
    }
    Ok(())
}

#[cfg(not(windows))]
fn shell_open(_path: &Path) -> Result<(), String> {
    Err("Apertura workspace disponibile soltanto su Windows.".to_string())
}

fn path_within(path: &Path, root: &Path) -> bool {
    let path = normalized_path(path);
    let root = normalized_path(root);
    path.starts_with(&format!("{root}\\"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalized_path(left) == normalized_path(right)
}

fn normalized_path(path: &Path) -> String {
    let mut value = path
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase();
    if let Some(stripped) = value.strip_prefix("\\\\?\\") {
        value = stripped.to_string();
    }
    value
}

fn relative_display(root: &Path, target: &Path) -> Result<String, String> {
    target
        .strip_prefix(root)
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .map_err(|_| "Percorso fuori workspace.".to_string())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn audit_workspace(
    state: &AppState,
    id: &str,
    capability: &str,
    decision: &str,
    summary: &str,
) -> Result<(), String> {
    append_audit(
        state,
        id,
        "tool.executed",
        decision,
        summary,
        &serde_json::json!({"capability":capability}).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        atomic_write, is_executable_like, path_within, relative_path, resolve_existing, same_path,
    };
    use std::{fs, path::Path};

    #[test]
    fn rejects_absolute_and_parent_traversal() {
        assert!(relative_path("../secret.txt", false).is_err());
        assert!(relative_path("C:\\Windows\\system.ini", false).is_err());
        assert!(relative_path("folder/file.txt", false).is_ok());
    }

    #[test]
    fn canonical_resolution_cannot_escape_root() {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path().join("project");
        fs::create_dir(&root).expect("root");
        fs::write(root.join("file.txt"), "ok").expect("file");
        assert!(resolve_existing(&root, "file.txt", false).is_ok());
        assert!(resolve_existing(&root, "../outside.txt", false).is_err());
        assert!(path_within(&root.join("folder"), &root));
    }

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_backup() {
        let temp = tempfile::tempdir().expect("temp");
        let target = temp.path().join("file.txt");
        fs::write(&target, "old").expect("old");
        atomic_write(&target, b"new").expect("write");
        assert_eq!(fs::read_to_string(&target).expect("read"), "new");
        assert_eq!(fs::read_dir(temp.path()).expect("dir").count(), 1);
    }

    #[test]
    fn executable_documents_are_not_opened_from_workspace() {
        assert!(is_executable_like(Path::new("setup.exe")));
        assert!(is_executable_like(Path::new("run.ps1")));
        assert!(!is_executable_like(Path::new("notes.txt")));
    }

    #[test]
    fn windows_extended_prefix_does_not_bypass_scope_comparison() {
        assert!(same_path(
            Path::new(r"\\?\C:\Users\Example"),
            Path::new(r"C:\Users\Example")
        ));
        assert!(path_within(
            Path::new(r"\\?\C:\Windows\System32"),
            Path::new(r"C:\Windows")
        ));
    }
}
