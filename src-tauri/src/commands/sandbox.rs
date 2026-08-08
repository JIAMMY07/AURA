use super::{
    audit::append_audit, pc::confirm_native_action, policy::enforce_capability, storage::AppState,
};
use crate::crypto::Crypto;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::State;
use uuid::Uuid;
use wait_timeout::ChildExt;

const MAX_REPORT_BYTES: u64 = 256 * 1024;
const MAX_VISIBLE_PROPOSALS: usize = 20;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxStatus {
    strong_isolation_available: bool,
    windows_sandbox: bool,
    wsl: bool,
    bubblewrap: bool,
    container_runtime: bool,
    simulation_available: bool,
    execution_allowed: bool,
    reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionProposal {
    id: String,
    report_path: String,
    executable: bool,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionOperation {
    id: String,
    status: String,
    artifact_path: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredEvolutionReport {
    version: u8,
    id: String,
    project_name: String,
    objective: String,
    sandbox: StoredSandboxStatus,
    state: String,
    promotion: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    artifact_path: String,
    #[serde(default)]
    run_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredSandboxStatus {
    strong_isolation_available: bool,
    execution_allowed: bool,
    reason: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct EncryptedReportEnvelope {
    version: u8,
    id: String,
    encryption: String,
    ciphertext: String,
    nonce: String,
    content_logged: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionReportView {
    id: String,
    project_name: String,
    objective: String,
    state: String,
    promotion: String,
    created_at: String,
    strong_isolation_available: bool,
    execution_allowed: bool,
    artifact_path: String,
    run_id: String,
}

#[tauri::command]
pub fn sandbox_status() -> SandboxStatus {
    detect_status()
}

#[tauri::command]
pub fn evolution_list_proposals(
    state: State<'_, AppState>,
) -> Result<Vec<EvolutionReportView>, String> {
    enforce_capability("evolution.inspect", "allow")?;
    let reports = state.app_data.join("evolution").join("reports");
    if !reports.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&reports)
        .map_err(|_| "Cartella report Evolution Lab non leggibile.".to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| {
        std::cmp::Reverse(
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH),
        )
    });
    paths
        .into_iter()
        .take(MAX_VISIBLE_PROPOSALS)
        .map(|path| decrypt_report_file(&path, &state.crypto))
        .collect()
}

#[tauri::command]
pub fn evolution_record_proposal(
    project_name: String,
    objective: String,
    state: State<'_, AppState>,
) -> Result<EvolutionProposal, String> {
    enforce_capability("evolution.propose", "sandbox")?;
    validate_text(&project_name, 128, "Nome progetto")?;
    validate_text(&objective, 8_192, "Obiettivo")?;
    let status = detect_status();
    let id = Uuid::new_v4().to_string();
    let reports = state.app_data.join("evolution").join("reports");
    fs::create_dir_all(&reports)
        .map_err(|_| "Cartella report Evolution Lab non disponibile.".to_string())?;
    let report_path = reports.join(format!("{id}.json"));
    let sensitive_report = StoredEvolutionReport {
        version: 1,
        id: id.clone(),
        project_name,
        objective,
        sandbox: StoredSandboxStatus {
            strong_isolation_available: status.strong_isolation_available,
            execution_allowed: status.execution_allowed,
            reason: status.reason,
        },
        state: "proposal-only".to_string(),
        promotion: "manual-review-required".to_string(),
        created_at: now_millis()?,
        artifact_path: String::new(),
        run_id: String::new(),
    };
    fs::write(
        &report_path,
        encrypt_report(&sensitive_report, &state.crypto)?,
    )
    .map_err(|_| "Scrittura report Evolution Lab fallita.".to_string())?;
    append_audit(
        &state,
        &id,
        "evolution.proposed",
        "sandbox",
        "Proposta registrata senza esecuzione",
        r#"{"contentLogged":false,"executable":false}"#,
    )?;
    Ok(EvolutionProposal {
        id,
        report_path: report_path.to_string_lossy().to_string(),
        executable: false,
        status: "proposal-only".to_string(),
    })
}

#[tauri::command]
pub async fn evolution_generate_patch(
    proposal_id: String,
    project_path: String,
    state: State<'_, AppState>,
) -> Result<EvolutionOperation, String> {
    enforce_capability("evolution.propose", "sandbox")?;
    validate_proposal_id(&proposal_id)?;
    let project = validate_project_path(&project_path)?;
    let objective = load_stored_report(
        &proposal_report_path(&state.app_data, &proposal_id),
        &proposal_id,
        &state.crypto,
    )?
    .objective;
    let script = ensure_runtime_script(
        &state.app_data,
        "evolution-generate-patch.ps1",
        include_str!("../../../scripts/evolution-generate-patch.ps1"),
    )?;
    let id = proposal_id.clone();
    let project_string = project.to_string_lossy().into_owned();
    tauri::async_runtime::spawn_blocking(move || {
        run_powershell(
            &script,
            vec![
                "-ProjectPath".to_string(),
                project_string,
                "-Objective".to_string(),
                objective,
                "-ProposalId".to_string(),
                id,
                "-Model".to_string(),
                "qwen2.5-coder:3b".to_string(),
            ],
            Duration::from_secs(330),
        )
    })
    .await
    .map_err(|_| "Worker generazione Evolution interrotto.".to_string())??;
    let patch = local_evolution_root()?
        .join("proposals")
        .join(&proposal_id)
        .join("proposal.patch");
    if !patch.is_file()
        || fs::metadata(&patch)
            .map_err(|_| "Patch non leggibile.".to_string())?
            .len()
            > 5 * 1024 * 1024
    {
        return Err("La generazione non ha prodotto una patch valida e limitata.".to_string());
    }
    update_report_state(
        &state,
        &proposal_id,
        "generated-not-executed",
        "manual-review-required",
        Some(&patch),
        None,
    )?;
    append_audit(
        &state,
        &proposal_id,
        "evolution.patch.generated",
        "sandbox",
        "Patch locale generata, non eseguita",
        r#"{"contentLogged":false}"#,
    )?;
    Ok(EvolutionOperation {
        id: proposal_id,
        status: "generated-not-executed".to_string(),
        artifact_path: patch.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn evolution_run_sandbox(
    proposal_id: String,
    project_path: String,
    test_profile: String,
    state: State<'_, AppState>,
) -> Result<EvolutionOperation, String> {
    enforce_capability("evolution.propose", "sandbox")?;
    validate_proposal_id(&proposal_id)?;
    if !detect_status().execution_allowed {
        return Err("Sandbox forte non verificata: esecuzione bloccata in sicurezza.".to_string());
    }
    if !matches!(test_profile.as_str(), "Static" | "Node" | "Rust") {
        return Err("Profilo test non consentito.".to_string());
    }
    let project = validate_project_path(&project_path)?;
    let patch = local_evolution_root()?
        .join("proposals")
        .join(&proposal_id)
        .join("proposal.patch");
    if !patch.is_file() {
        return Err("Genera prima la patch della proposta.".to_string());
    }
    let run_id = Uuid::new_v4().to_string();
    let script = ensure_runtime_script(
        &state.app_data,
        "evolution-sandbox-run.ps1",
        include_str!("../../../scripts/evolution-sandbox-run.ps1"),
    )?;
    let run_copy = run_id.clone();
    let project_string = project.to_string_lossy().into_owned();
    let patch_string = patch.to_string_lossy().into_owned();
    tauri::async_runtime::spawn_blocking(move || {
        run_powershell(
            &script,
            vec![
                "-ProjectPath".to_string(),
                project_string,
                "-PatchPath".to_string(),
                patch_string,
                "-TestProfile".to_string(),
                test_profile,
                "-RunId".to_string(),
                run_copy,
            ],
            Duration::from_secs(720),
        )
    })
    .await
    .map_err(|_| "Worker sandbox Evolution interrotto.".to_string())??;
    let validated = local_evolution_root()?
        .join("runs")
        .join(&run_id)
        .join("validated.patch");
    if !validated.is_file() {
        return Err("La sandbox non ha prodotto una patch validata.".to_string());
    }
    update_report_state(
        &state,
        &proposal_id,
        "sandbox-validated",
        "manual-review-required",
        Some(&validated),
        Some(&run_id),
    )?;
    append_audit(
        &state,
        &proposal_id,
        "evolution.sandbox.validated",
        "sandbox",
        "Patch validata nel runtime isolato",
        &serde_json::json!({"runId":run_id}).to_string(),
    )?;
    Ok(EvolutionOperation {
        id: proposal_id,
        status: "sandbox-validated".to_string(),
        artifact_path: validated.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn evolution_promote_patch(
    proposal_id: String,
    project_path: String,
    validated_patch: String,
    state: State<'_, AppState>,
) -> Result<EvolutionOperation, String> {
    enforce_capability("evolution.propose", "sandbox")?;
    validate_proposal_id(&proposal_id)?;
    if !detect_status().execution_allowed {
        return Err("Sandbox forte non verificata: promozione bloccata.".to_string());
    }
    if !confirm_native_action("Conferma promozione AURA", "Applicare la patch validata a un nuovo branch Git di revisione? Nessun commit verra creato.") {
        return Err("Promozione annullata dall'utente.".to_string());
    }
    let project = validate_project_path(&project_path)?;
    let validated = fs::canonicalize(&validated_patch)
        .map_err(|_| "Patch validata non trovata.".to_string())?;
    let expected_root = fs::canonicalize(local_evolution_root()?.join("runs"))
        .map_err(|_| "Cartella run non disponibile.".to_string())?;
    if !validated.starts_with(&expected_root)
        || validated.file_name().and_then(|v| v.to_str()) != Some("validated.patch")
    {
        return Err("La patch non proviene da un run Evolution validato.".to_string());
    }
    let script = ensure_runtime_script(
        &state.app_data,
        "evolution-promote-patch.ps1",
        include_str!("../../../scripts/evolution-promote-patch.ps1"),
    )?;
    let project_string = project.to_string_lossy().into_owned();
    let validated_string = validated.to_string_lossy().into_owned();
    tauri::async_runtime::spawn_blocking(move || {
        run_powershell(
            &script,
            vec![
                "-ProjectPath".to_string(),
                project_string,
                "-ValidatedPatch".to_string(),
                validated_string,
                "-Approve".to_string(),
            ],
            Duration::from_secs(60),
        )
    })
    .await
    .map_err(|_| "Worker promozione Evolution interrotto.".to_string())??;
    update_report_state(
        &state,
        &proposal_id,
        "promoted-to-review-branch",
        "human-review-pending",
        None,
        None,
    )?;
    append_audit(
        &state,
        &proposal_id,
        "evolution.promoted",
        "confirm",
        "Patch applicata a branch Git di revisione",
        r#"{"commitCreated":false}"#,
    )?;
    Ok(EvolutionOperation {
        id: proposal_id,
        status: "promoted-to-review-branch".to_string(),
        artifact_path: validated_patch,
    })
}

fn encrypt_report(report: &StoredEvolutionReport, crypto: &Crypto) -> Result<Vec<u8>, String> {
    let aad = format!("aura-evolution-report:v1:{}", report.id);
    let plaintext = serde_json::to_string(report)
        .map_err(|_| "Report Evolution Lab non serializzabile.".to_string())?;
    let (ciphertext, nonce) = crypto.encrypt_with_aad(&plaintext, aad.as_bytes())?;
    serde_json::to_vec_pretty(&EncryptedReportEnvelope {
        version: 1,
        id: report.id.clone(),
        encryption: "AES-256-GCM".to_string(),
        ciphertext: STANDARD.encode(ciphertext),
        nonce: STANDARD.encode(nonce),
        content_logged: false,
    })
    .map_err(|_| "Report Evolution Lab non serializzabile.".to_string())
}

fn decrypt_report_file(path: &Path, crypto: &Crypto) -> Result<EvolutionReportView, String> {
    let expected_id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Nome report Evolution Lab non valido.".to_string())?;
    Uuid::parse_str(expected_id).map_err(|_| "Identificatore report non valido.".to_string())?;
    let metadata =
        fs::metadata(path).map_err(|_| "Report Evolution Lab non leggibile.".to_string())?;
    if metadata.len() > MAX_REPORT_BYTES {
        return Err("Report Evolution Lab oltre il limite consentito.".to_string());
    }
    let bytes = fs::read(path).map_err(|_| "Report Evolution Lab non leggibile.".to_string())?;
    decrypt_report(&bytes, expected_id, crypto)
}

fn decrypt_report(
    bytes: &[u8],
    expected_id: &str,
    crypto: &Crypto,
) -> Result<EvolutionReportView, String> {
    let envelope: EncryptedReportEnvelope = serde_json::from_slice(bytes)
        .map_err(|_| "Busta report Evolution Lab non valida.".to_string())?;
    if envelope.version != 1
        || envelope.encryption != "AES-256-GCM"
        || envelope.content_logged
        || envelope.id != expected_id
    {
        return Err("Metadati report Evolution Lab non validi.".to_string());
    }
    let ciphertext = STANDARD
        .decode(envelope.ciphertext)
        .map_err(|_| "Ciphertext report non valido.".to_string())?;
    let nonce = STANDARD
        .decode(envelope.nonce)
        .map_err(|_| "Nonce report non valido.".to_string())?;
    let aad = format!("aura-evolution-report:v1:{expected_id}");
    let plaintext = crypto.decrypt_with_aad(&ciphertext, &nonce, aad.as_bytes())?;
    let report: StoredEvolutionReport = serde_json::from_str(&plaintext)
        .map_err(|_| "Contenuto report Evolution Lab non valido.".to_string())?;
    if report.version != 1 || report.id != expected_id {
        return Err("Integrità identificatore report non valida.".to_string());
    }
    Ok(EvolutionReportView {
        id: report.id,
        project_name: report.project_name,
        objective: report.objective,
        state: report.state,
        promotion: report.promotion,
        created_at: report.created_at,
        strong_isolation_available: report.sandbox.strong_isolation_available,
        execution_allowed: report.sandbox.execution_allowed,
        artifact_path: report.artifact_path,
        run_id: report.run_id,
    })
}

fn load_stored_report(
    path: &Path,
    expected_id: &str,
    crypto: &Crypto,
) -> Result<StoredEvolutionReport, String> {
    let metadata =
        fs::metadata(path).map_err(|_| "Report Evolution Lab non leggibile.".to_string())?;
    if metadata.len() > MAX_REPORT_BYTES {
        return Err("Report Evolution Lab oltre il limite consentito.".to_string());
    }
    let envelope: EncryptedReportEnvelope = serde_json::from_slice(
        &fs::read(path).map_err(|_| "Report Evolution Lab non leggibile.".to_string())?,
    )
    .map_err(|_| "Busta report Evolution Lab non valida.".to_string())?;
    if envelope.version != 1
        || envelope.encryption != "AES-256-GCM"
        || envelope.content_logged
        || envelope.id != expected_id
    {
        return Err("Metadati report Evolution Lab non validi.".to_string());
    }
    let ciphertext = STANDARD
        .decode(envelope.ciphertext)
        .map_err(|_| "Ciphertext report non valido.".to_string())?;
    let nonce = STANDARD
        .decode(envelope.nonce)
        .map_err(|_| "Nonce report non valido.".to_string())?;
    let aad = format!("aura-evolution-report:v1:{expected_id}");
    let plaintext = crypto.decrypt_with_aad(&ciphertext, &nonce, aad.as_bytes())?;
    let report: StoredEvolutionReport = serde_json::from_str(&plaintext)
        .map_err(|_| "Contenuto report Evolution Lab non valido.".to_string())?;
    if report.version != 1 || report.id != expected_id {
        return Err("Integrita identificatore report non valida.".to_string());
    }
    Ok(report)
}

fn proposal_report_path(app_data: &Path, proposal_id: &str) -> PathBuf {
    app_data
        .join("evolution")
        .join("reports")
        .join(format!("{proposal_id}.json"))
}

fn update_report_state(
    state: &AppState,
    proposal_id: &str,
    new_state: &str,
    promotion: &str,
    artifact_path: Option<&Path>,
    run_id: Option<&str>,
) -> Result<(), String> {
    let path = proposal_report_path(&state.app_data, proposal_id);
    let mut report = load_stored_report(&path, proposal_id, &state.crypto)?;
    report.state = new_state.to_string();
    report.promotion = promotion.to_string();
    if let Some(path) = artifact_path {
        report.artifact_path = path.to_string_lossy().to_string();
    }
    if let Some(value) = run_id {
        report.run_id = value.to_string();
    }
    fs::write(path, encrypt_report(&report, &state.crypto)?)
        .map_err(|_| "Aggiornamento report Evolution fallito.".to_string())
}

fn validate_proposal_id(value: &str) -> Result<(), String> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| "Identificatore proposta non valido.".to_string())
}

fn validate_project_path(value: &str) -> Result<PathBuf, String> {
    if value.trim().is_empty() || value.len() > 1024 {
        return Err("Percorso progetto fuori dai limiti.".to_string());
    }
    let project =
        fs::canonicalize(value).map_err(|_| "Percorso progetto non trovato.".to_string())?;
    if !project.is_dir() || !project.join(".git").exists() {
        return Err("Seleziona la radice di un repository Git.".to_string());
    }
    Ok(project)
}

fn local_evolution_root() -> Result<PathBuf, String> {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| root.join("AURA").join("evolution"))
        .ok_or_else(|| "LOCALAPPDATA non disponibile.".to_string())
}

fn ensure_runtime_script(app_data: &Path, name: &str, contents: &str) -> Result<PathBuf, String> {
    if !matches!(
        name,
        "evolution-generate-patch.ps1"
            | "evolution-sandbox-run.ps1"
            | "evolution-promote-patch.ps1"
    ) {
        return Err("Script Evolution non autorizzato.".to_string());
    }
    let root = app_data.join("evolution").join("runtime");
    fs::create_dir_all(&root).map_err(|_| "Runtime Evolution non scrivibile.".to_string())?;
    let path = root.join(name);
    if fs::read_to_string(&path).ok().as_deref() != Some(contents) {
        fs::write(&path, contents.as_bytes())
            .map_err(|_| "Installazione script Evolution fallita.".to_string())?;
    }
    Ok(path)
}

fn run_powershell(script: &Path, arguments: Vec<String>, timeout: Duration) -> Result<(), String> {
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Avvio pipeline Evolution fallito.".to_string())?;
    match child
        .wait_timeout(timeout)
        .map_err(|_| "Attesa pipeline Evolution fallita.".to_string())?
    {
        Some(status) if status.success() => Ok(()),
        Some(status) => Err(format!(
            "Pipeline Evolution fallita con codice {}.",
            status.code().unwrap_or(-1)
        )),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Err("Pipeline Evolution oltre il limite di tempo.".to_string())
        }
    }
}

fn detect_status() -> SandboxStatus {
    let windows_sandbox = Path::new(r"C:\Windows\System32\WindowsSandbox.exe").is_file();
    let wsl = Command::new("wsl.exe")
        .args(["--list", "--quiet"])
        .output()
        .map(|output| {
            output.status.success()
                && output
                    .stdout
                    .iter()
                    .any(|byte| !byte.is_ascii_whitespace() && *byte != 0)
        })
        .unwrap_or(false);
    let container_runtime = runtime_available("docker.exe") || runtime_available("podman.exe");
    let bubblewrap = wsl && verified_bubblewrap();
    let strong = strong_isolation(bubblewrap);
    SandboxStatus {
        strong_isolation_available: strong,
        windows_sandbox,
        wsl,
        bubblewrap,
        container_runtime,
        simulation_available: true,
        execution_allowed: strong,
        reason: if bubblewrap {
            "WSL+bubblewrap verificato: host e rete non sono montati; il runner può eseguire soltanto profili di test allowlistati."
        } else {
            "Sandbox forte non verificata: consentite solo proposta, diff e revisione manuale. Eseguire setup-evolution-sandbox.ps1 come amministratore."
        }
        .to_string(),
    }
}

fn strong_isolation(verified_bubblewrap: bool) -> bool {
    verified_bubblewrap
}

fn verified_bubblewrap() -> bool {
    let mut command = Command::new("wsl.exe");
    command.args([
        "--distribution",
        "Ubuntu",
        "--user",
        "root",
        "--exec",
        "bwrap",
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--ro-bind",
        "/usr",
        "/usr",
        "--ro-bind",
        "/bin",
        "/bin",
        "--ro-bind",
        "/lib",
        "/lib",
        "--ro-bind-try",
        "/lib64",
        "/lib64",
        "--ro-bind",
        "/etc",
        "/etc",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
        "--chdir",
        "/tmp",
        "/bin/sh",
        "-lc",
        "test \"$(cat /etc/aura-sandbox-version 2>/dev/null)\" = AURA_WSL_BWRAP_V1 && test ! -e /mnt/c && touch /tmp/aura-write-test",
    ]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    match child.wait_timeout(Duration::from_secs(8)) {
        Ok(Some(status)) => status.success(),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            false
        }
    }
}

fn runtime_available(name: &str) -> bool {
    if !executable_on_path(name) {
        return false;
    }
    let mut command = Command::new(name);
    command
        .arg("info")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    match child.wait_timeout(Duration::from_secs(3)) {
        Ok(Some(status)) => status.success(),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            false
        }
    }
}

fn executable_on_path(name: &str) -> bool {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .any(|path| path.join(name).is_file())
}

fn validate_text(value: &str, maximum: usize, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > maximum {
        Err(format!("{label} fuori dai limiti consentiti."))
    } else {
        Ok(())
    }
}

fn now_millis() -> Result<String, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .map_err(|_| "Orologio di sistema non valido.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        decrypt_report, detect_status, encrypt_report, strong_isolation, validate_text,
        StoredEvolutionReport, StoredSandboxStatus,
    };
    use crate::crypto::Crypto;
    use uuid::Uuid;

    #[test]
    fn rejects_empty_or_oversized_proposals() {
        assert!(validate_text("", 10, "x").is_err());
        assert!(validate_text("troppo lungo", 3, "x").is_err());
        assert!(validate_text("ok", 10, "x").is_ok());
    }

    #[test]
    fn execution_requires_verified_bubblewrap() {
        let detected = detect_status();
        assert_eq!(detected.execution_allowed, detected.bubblewrap);
        assert!(strong_isolation(true));
        assert!(!strong_isolation(false));
    }

    #[test]
    fn encrypted_reports_round_trip_and_reject_wrong_identity() {
        let id = Uuid::new_v4().to_string();
        let report = StoredEvolutionReport {
            version: 1,
            id: id.clone(),
            project_name: "progetto sintetico".to_string(),
            objective: "obiettivo riservato sintetico".to_string(),
            sandbox: StoredSandboxStatus {
                strong_isolation_available: false,
                execution_allowed: false,
                reason: "test".to_string(),
            },
            state: "proposal-only".to_string(),
            promotion: "manual-review-required".to_string(),
            created_at: "1".to_string(),
            artifact_path: String::new(),
            run_id: String::new(),
        };
        let crypto = Crypto::from_master([23_u8; 32]).expect("crypto");
        let encrypted = encrypt_report(&report, &crypto).expect("encrypt report");
        assert!(!String::from_utf8_lossy(&encrypted).contains(&report.objective));
        let view = decrypt_report(&encrypted, &id, &crypto).expect("decrypt report");
        assert_eq!(view.objective, report.objective);
        assert!(decrypt_report(&encrypted, &Uuid::new_v4().to_string(), &crypto).is_err());
    }
}
