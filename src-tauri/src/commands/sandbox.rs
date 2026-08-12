use super::{
    audit::append_audit, pc::confirm_native_action, policy::enforce_capability, storage::AppState,
};
use crate::crypto::Crypto;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
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
    canary_available: bool,
    automatic_rollback: bool,
    attestation_version: u8,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxInstallResult {
    started: bool,
    restart_may_be_required: bool,
    message: String,
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
    #[serde(default)]
    inspection: Option<EvolutionInspection>,
    #[serde(default)]
    diff_approved: bool,
    #[serde(default)]
    attempts: u8,
    #[serde(default)]
    attestation: Option<EvolutionAttestation>,
    #[serde(default)]
    rollback_status: String,
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
    inspection: Option<EvolutionInspection>,
    diff_approved: bool,
    attempts: u8,
    attestation: Option<EvolutionAttestation>,
    rollback_status: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionAttestation {
    schema_version: u8,
    run_id: String,
    test_profile: String,
    patch_sha256: String,
    dependency_policy: String,
    network_during_test: bool,
    host_filesystem_mounted_read_only: bool,
    canary_passed: bool,
    canary_fresh_namespace: bool,
    canary_network_disabled: bool,
    #[serde(default)]
    changed_files: Vec<String>,
    #[serde(default)]
    security_findings: Vec<String>,
    created_at_utc: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CanaryAttestation {
    schema_version: u8,
    run_id: String,
    passed: bool,
    fresh_namespace: bool,
    network_during_canary: bool,
    host_filesystem_mounted_read_only: bool,
    exit_code: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromotionManifest {
    schema_version: u8,
    status: String,
    original_branch: String,
    original_commit: String,
    review_branch: String,
    patch_sha256: String,
    rollback_verified: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionInspection {
    changed_files: Vec<String>,
    capabilities: Vec<String>,
    dependency_changes: Vec<String>,
    security_findings: Vec<String>,
    risk: String,
    patch_sha256: String,
    diff_excerpt: String,
}

#[tauri::command]
pub fn sandbox_status() -> SandboxStatus {
    detect_status()
}

#[tauri::command]
pub fn evolution_detect_project_root() -> Result<Option<String>, String> {
    enforce_capability("evolution.inspect", "allow")?;
    let mut starts = Vec::new();
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            starts.push(parent.to_path_buf());
        }
    }
    if let Ok(current) = env::current_dir() {
        starts.push(current);
    }
    for start in starts {
        for ancestor in start.ancestors() {
            if ancestor.join(".git").is_dir()
                && ancestor.join("package.json").is_file()
                && ancestor.join("src-tauri").join("Cargo.toml").is_file()
            {
                return Ok(Some(ancestor.to_string_lossy().to_string()));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn sandbox_install_runtime(state: State<'_, AppState>) -> Result<SandboxInstallResult, String> {
    enforce_capability("sandbox.install", "confirm")?;
    if detect_status().execution_allowed {
        return Ok(SandboxInstallResult {
            started: false,
            restart_may_be_required: false,
            message: "Runtime di isolamento già verificato.".to_string(),
        });
    }
    if !confirm_native_action(
        "Installa runtime isolato AURA",
        "Abilitare WSL e Virtual Machine Platform e installare Ubuntu con bubblewrap? Windows mostrerà una richiesta UAC e potrebbe richiedere il riavvio.",
    ) {
        return Ok(SandboxInstallResult {
            started: false,
            restart_may_be_required: false,
            message: "Installazione runtime annullata.".to_string(),
        });
    }
    let script = ensure_runtime_script(
        &state.app_data,
        "setup-evolution-sandbox.ps1",
        include_str!("../../../scripts/setup-evolution-sandbox.ps1"),
    )?;
    launch_elevated_setup(&script)?;
    append_audit(
        &state,
        "sandbox-runtime-install",
        "sandbox.install.started",
        "confirm",
        "Provisioning runtime isolato avviato con UAC",
        r#"{"contentLogged":false,"restartMayBeRequired":true}"#,
    )?;
    Ok(SandboxInstallResult {
        started: true,
        restart_may_be_required: true,
        message: "Provisioning avviato. Segui la finestra amministrativa; se richiesto, riavvia Windows e premi di nuovo questo pulsante.".to_string(),
    })
}

#[cfg(windows)]
fn launch_elevated_setup(script: &Path) -> Result<(), String> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let verb = "runas\0".encode_utf16().collect::<Vec<_>>();
    let executable = "powershell.exe\0".encode_utf16().collect::<Vec<_>>();
    let parameters = format!(
        "-NoExit -NoProfile -ExecutionPolicy Bypass -File \"{}\"\0",
        script.to_string_lossy()
    )
    .encode_utf16()
    .collect::<Vec<_>>();
    // SAFETY: all strings are NUL-terminated UTF-16 buffers that remain alive
    // for the duration of ShellExecuteW; no mutable pointers are passed.
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            verb.as_ptr(),
            executable.as_ptr(),
            parameters.as_ptr(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    if result as usize <= 32 {
        Err("Windows non ha avviato il provisioning elevato.".to_string())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn launch_elevated_setup(_script: &Path) -> Result<(), String> {
    Err("Provisioning disponibile soltanto su Windows.".to_string())
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
        inspection: None,
        diff_approved: false,
        attempts: 0,
        attestation: None,
        rollback_status: String::new(),
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
    let report_path = proposal_report_path(&state.app_data, &proposal_id);
    let mut report = load_stored_report(&report_path, &proposal_id, &state.crypto)?;
    if !matches!(
        report.state.as_str(),
        "proposal-only" | "awaiting-diff-approval" | "diff-approved" | "generation-failed"
    ) {
        return Err("La proposta non e nello stato corretto per generare una patch.".to_string());
    }
    if report.attempts >= 3 {
        return Err(
            "Limite di tre tentativi raggiunto: crea una nuova proposta con un obiettivo corretto."
                .to_string(),
        );
    }
    let objective = report.objective.clone();
    report.state = "generating".to_string();
    report.promotion = "not-eligible".to_string();
    report.artifact_path.clear();
    report.inspection = None;
    report.diff_approved = false;
    report.attestation = None;
    report.rollback_status.clear();
    report.attempts = report.attempts.saturating_add(1);
    store_report(&report_path, &report, &state.crypto)?;
    let artifact_dir = local_evolution_root()?.join("proposals").join(&proposal_id);
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir).map_err(|error| {
            format!("Impossibile sostituire gli artefatti del tentativo precedente: {error}")
        })?;
    }
    let script = ensure_runtime_script(
        &state.app_data,
        "evolution-generate-patch.ps1",
        include_str!("../../../scripts/evolution-generate-patch.ps1"),
    )?;
    let id = proposal_id.clone();
    let project_string = project.to_string_lossy().into_owned();
    let generation = tauri::async_runtime::spawn_blocking(move || {
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
                "aura-coder:3b".to_string(),
            ],
            Duration::from_secs(330),
        )
    })
    .await
    .unwrap_or_else(|_| Err("Worker generazione Evolution interrotto.".to_string()));
    if let Err(error) = generation {
        report.state = "generation-failed".to_string();
        store_report(&report_path, &report, &state.crypto)?;
        append_audit(
            &state,
            &proposal_id,
            "evolution.patch.failed",
            "sandbox",
            "Tentativo di generazione fallito senza esecuzione",
            &serde_json::json!({"attempt":report.attempts,"contentLogged":false}).to_string(),
        )?;
        return Err(error);
    }
    let patch = local_evolution_root()?
        .join("proposals")
        .join(&proposal_id)
        .join("proposal.patch");
    let inspection = if !patch.is_file() {
        Err("La generazione non ha prodotto una patch valida e limitata.".to_string())
    } else {
        inspect_patch_file(&patch)
    };
    let inspection = match inspection {
        Ok(value) => value,
        Err(error) => {
            report.state = "generation-failed".to_string();
            store_report(&report_path, &report, &state.crypto)?;
            return Err(error);
        }
    };
    report.state = "awaiting-diff-approval".to_string();
    report.promotion = "diff-review-required".to_string();
    report.artifact_path = patch.to_string_lossy().to_string();
    report.inspection = Some(inspection.clone());
    report.diff_approved = false;
    store_report(&report_path, &report, &state.crypto)?;
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
        status: "awaiting-diff-approval".to_string(),
        artifact_path: patch.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn evolution_inspect_patch(
    proposal_id: String,
    state: State<'_, AppState>,
) -> Result<EvolutionInspection, String> {
    enforce_capability("evolution.inspect", "allow")?;
    validate_proposal_id(&proposal_id)?;
    let report_path = proposal_report_path(&state.app_data, &proposal_id);
    let report = load_stored_report(&report_path, &proposal_id, &state.crypto)?;
    report
        .inspection
        .ok_or_else(|| "La proposta non contiene ancora un diff ispezionabile.".to_string())
}

#[tauri::command]
pub fn evolution_approve_patch(
    proposal_id: String,
    state: State<'_, AppState>,
) -> Result<EvolutionOperation, String> {
    enforce_capability("evolution.approve", "confirm")?;
    validate_proposal_id(&proposal_id)?;
    let report_path = proposal_report_path(&state.app_data, &proposal_id);
    let mut report = load_stored_report(&report_path, &proposal_id, &state.crypto)?;
    if report.state != "awaiting-diff-approval" || report.inspection.is_none() {
        return Err("Genera e ispeziona il diff prima dell'approvazione.".to_string());
    }
    let inspection = report.inspection.as_ref().expect("inspection checked");
    let prompt = format!(
        "Approvare il diff generato da AURA Forge? Rischio: {}. File: {}. L'approvazione consente soltanto i test nella sandbox forte; non modifica il progetto reale.",
        inspection.risk,
        inspection.changed_files.join(", ")
    );
    if !confirm_native_action("Approva diff AURA Forge", &prompt) {
        return Err("Approvazione del diff annullata dall'utente.".to_string());
    }
    report.state = "diff-approved".to_string();
    report.promotion = "sandbox-validation-required".to_string();
    report.diff_approved = true;
    store_report(&report_path, &report, &state.crypto)?;
    append_audit(
        &state,
        &proposal_id,
        "evolution.diff.approved",
        "confirm",
        "Diff effettivo approvato per il test isolato",
        &serde_json::json!({
            "risk": inspection.risk,
            "patchSha256": inspection.patch_sha256,
            "contentLogged": false
        })
        .to_string(),
    )?;
    Ok(EvolutionOperation {
        id: proposal_id,
        status: "diff-approved".to_string(),
        artifact_path: report.artifact_path,
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
    let report_path = proposal_report_path(&state.app_data, &proposal_id);
    let mut report = load_stored_report(&report_path, &proposal_id, &state.crypto)?;
    if report.state != "diff-approved" || !report.diff_approved {
        return Err("Il diff effettivo deve essere approvato prima dei test.".to_string());
    }
    let patch = local_evolution_root()?
        .join("proposals")
        .join(&proposal_id)
        .join("proposal.patch");
    if !patch.is_file() {
        return Err("Genera prima la patch della proposta.".to_string());
    }
    let approved_hash = report
        .inspection
        .as_ref()
        .map(|inspection| inspection.patch_sha256.as_str())
        .ok_or_else(|| "Ispezione approvata assente.".to_string())?;
    if sha256_file(&patch)? != approved_hash {
        return Err("La patch è cambiata dopo l'approvazione: test bloccati.".to_string());
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
    let profile_for_runner = test_profile.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_powershell(
            &script,
            vec![
                "-ProjectPath".to_string(),
                project_string,
                "-PatchPath".to_string(),
                patch_string,
                "-TestProfile".to_string(),
                profile_for_runner,
                "-RunId".to_string(),
                run_copy,
            ],
            Duration::from_secs(720),
        )
    })
    .await
    .map_err(|_| "Worker sandbox Evolution interrotto.".to_string())??;
    let run_root = local_evolution_root()?.join("runs").join(&run_id);
    let validated = run_root.join("validated.patch");
    if !validated.is_file() {
        return Err("La sandbox non ha prodotto una patch validata.".to_string());
    }
    let attestation = verify_run_attestation(&run_root, &report, &run_id, &test_profile)?;
    report.state = "canary-validated".to_string();
    report.promotion = "manual-promotion-with-auto-rollback".to_string();
    report.artifact_path = validated.to_string_lossy().to_string();
    report.run_id = run_id.clone();
    report.attestation = Some(attestation);
    report.rollback_status = "armed".to_string();
    store_report(&report_path, &report, &state.crypto)?;
    append_audit(
        &state,
        &proposal_id,
        "evolution.sandbox.validated",
        "sandbox",
        "Patch validata da test e canary in runtime isolati distinti",
        &serde_json::json!({"runId":run_id,"canaryPassed":true,"rollbackArmed":true}).to_string(),
    )?;
    Ok(EvolutionOperation {
        id: proposal_id,
        status: "canary-validated".to_string(),
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
    let report_path = proposal_report_path(&state.app_data, &proposal_id);
    let mut report = load_stored_report(&report_path, &proposal_id, &state.crypto)?;
    if report.state != "canary-validated"
        || report.run_id.is_empty()
        || !report.diff_approved
        || report
            .attestation
            .as_ref()
            .is_none_or(|value| !value.canary_passed)
    {
        return Err("La proposta non risulta validata da un run sandbox correlato.".to_string());
    }
    let validated = fs::canonicalize(&validated_patch)
        .map_err(|_| "Patch validata non trovata.".to_string())?;
    let expected = fs::canonicalize(
        local_evolution_root()?
            .join("runs")
            .join(&report.run_id)
            .join("validated.patch"),
    )
    .map_err(|_| "Artefatto del run correlato non disponibile.".to_string())?;
    let reported = fs::canonicalize(&report.artifact_path)
        .map_err(|_| "Artefatto registrato nel report non disponibile.".to_string())?;
    if validated != expected || validated != reported {
        return Err("La patch non corrisponde al run associato alla proposta.".to_string());
    }
    if !confirm_native_action("Conferma promozione AURA", "Applicare la patch validata a un nuovo branch Git di revisione? Nessun commit verra creato.") {
        return Err("Promozione annullata dall'utente.".to_string());
    }
    let project = validate_project_path(&project_path)?;
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
    let promotion_result = tauri::async_runtime::spawn_blocking(move || {
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
    .map_err(|_| "Worker promozione Evolution interrotto.".to_string())?;
    if let Err(reason) = promotion_result {
        let rollback = local_evolution_root()?
            .join("runs")
            .join(&report.run_id)
            .join("rollback.json");
        let rollback_failed = local_evolution_root()?
            .join("runs")
            .join(&report.run_id)
            .join("rollback-failed.json");
        report.state = if rollback.is_file() {
            "promotion-rolled-back"
        } else if rollback_failed.is_file() {
            "rollback-failed-manual-recovery-required"
        } else {
            "promotion-blocked-no-change"
        }
        .to_string();
        report.promotion = "review-required-before-retry".to_string();
        report.rollback_status = if rollback.is_file() {
            "completed"
        } else if rollback_failed.is_file() {
            "failed"
        } else {
            "not-needed"
        }
        .to_string();
        store_report(&report_path, &report, &state.crypto)?;
        append_audit(
            &state,
            &proposal_id,
            "evolution.promotion.rolled-back",
            "deny",
            "Promozione non completata; esito rollback registrato senza occultare i fallimenti",
            &serde_json::json!({"rollback":report.rollback_status,"reason":reason}).to_string(),
        )?;
        return Err(reason);
    }
    let manifest_path = local_evolution_root()?
        .join("runs")
        .join(&report.run_id)
        .join("promotion-manifest.json");
    let manifest: PromotionManifest = serde_json::from_slice(
        &fs::read(&manifest_path)
            .map_err(|_| "Manifest di recupero promozione assente.".to_string())?,
    )
    .map_err(|_| "Manifest di recupero promozione non valido.".to_string())?;
    let expected_hash = report
        .attestation
        .as_ref()
        .map(|value| value.patch_sha256.as_str());
    if manifest.schema_version != 1
        || manifest.status != "canary-promoted"
        || manifest.original_branch.is_empty()
        || manifest.original_commit.len() != 40
        || manifest.review_branch.is_empty()
        || Some(manifest.patch_sha256.as_str()) != expected_hash
        || !manifest.rollback_verified
    {
        return Err("Manifest di recupero incoerente: stato non promosso.".to_string());
    }
    report.state = "canary-promoted-to-review-branch".to_string();
    report.promotion = "human-review-pending".to_string();
    report.rollback_status = "verified".to_string();
    store_report(&report_path, &report, &state.crypto)?;
    append_audit(
        &state,
        &proposal_id,
        "evolution.promoted",
        "confirm",
        "Patch canary applicata a branch Git di revisione con rollback verificato",
        r#"{"commitCreated":false,"canaryPassed":true,"rollbackVerified":true}"#,
    )?;
    Ok(EvolutionOperation {
        id: proposal_id,
        status: "canary-promoted-to-review-branch".to_string(),
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
        inspection: report.inspection,
        diff_approved: report.diff_approved,
        attempts: report.attempts,
        attestation: report.attestation,
        rollback_status: report.rollback_status,
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

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|_| "Artefatto Evolution non leggibile.".to_string())?;
    if bytes.is_empty() || bytes.len() > 5 * 1024 * 1024 {
        return Err("Artefatto Evolution vuoto o oltre 5 MiB.".to_string());
    }
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn verify_run_attestation(
    run_root: &Path,
    report: &StoredEvolutionReport,
    run_id: &str,
    test_profile: &str,
) -> Result<EvolutionAttestation, String> {
    let attestation_path = run_root.join("attestation.json");
    let canary_path = run_root.join("canary.json");
    for path in [&attestation_path, &canary_path] {
        let metadata =
            fs::metadata(path).map_err(|_| "Attestazione sandbox/canary mancante.".to_string())?;
        if metadata.len() == 0 || metadata.len() > MAX_REPORT_BYTES {
            return Err("Attestazione sandbox/canary fuori dai limiti.".to_string());
        }
    }
    let attestation: EvolutionAttestation = serde_json::from_slice(
        &fs::read(&attestation_path)
            .map_err(|_| "Attestazione sandbox non leggibile.".to_string())?,
    )
    .map_err(|_| "Attestazione sandbox non valida.".to_string())?;
    let canary: CanaryAttestation = serde_json::from_slice(
        &fs::read(&canary_path).map_err(|_| "Attestazione canary non leggibile.".to_string())?,
    )
    .map_err(|_| "Attestazione canary non valida.".to_string())?;
    let validated = run_root.join("validated.patch");
    let actual_hash = sha256_file(&validated)?;
    let approved_hash = report
        .inspection
        .as_ref()
        .map(|inspection| inspection.patch_sha256.as_str())
        .ok_or_else(|| "Hash della patch approvata assente.".to_string())?;
    if attestation.schema_version != 2
        || attestation.run_id != run_id
        || attestation.test_profile != test_profile
        || attestation.patch_sha256 != actual_hash
        || actual_hash != approved_hash
        || attestation.dependency_policy != "immutable-offline"
        || attestation.network_during_test
        || !attestation.host_filesystem_mounted_read_only
        || !attestation.canary_passed
        || !attestation.canary_fresh_namespace
        || !attestation.canary_network_disabled
        || canary.schema_version != 1
        || canary.run_id != run_id
        || !canary.passed
        || !canary.fresh_namespace
        || canary.network_during_canary
        || !canary.host_filesystem_mounted_read_only
        || canary.exit_code != 0
    {
        return Err(
            "Attestazione non conforme: artefatto revocato e promozione bloccata.".to_string(),
        );
    }
    Ok(attestation)
}

fn store_report(
    path: &Path,
    report: &StoredEvolutionReport,
    crypto: &Crypto,
) -> Result<(), String> {
    fs::write(path, encrypt_report(report, crypto)?)
        .map_err(|_| "Aggiornamento report Evolution fallito.".to_string())
}

fn inspect_patch_file(path: &Path) -> Result<EvolutionInspection, String> {
    let bytes = fs::read(path).map_err(|_| "Patch Evolution non leggibile.".to_string())?;
    if bytes.is_empty() || bytes.len() > 5 * 1024 * 1024 {
        return Err("Patch Evolution vuota o oltre i limiti.".to_string());
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "Patch Evolution non codificata in UTF-8 valido.".to_string())?;
    let mut changed_files = BTreeSet::new();
    let mut capabilities = BTreeSet::new();
    let mut dependency_changes = BTreeSet::new();
    let mut security_findings = BTreeSet::new();
    let mut added = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            if let Some((file, _)) = rest.split_once(" b/") {
                changed_files.insert(file.to_string());
                if matches!(
                    file,
                    "package.json" | "pnpm-lock.yaml" | "Cargo.toml" | "Cargo.lock"
                ) || file.ends_with("requirements.txt")
                    || file.ends_with("pyproject.toml")
                {
                    dependency_changes.insert(file.to_string());
                }
            }
        }
        if line.starts_with('+') && !line.starts_with("+++") {
            added.push_str(line);
            added.push('\n');
        }
    }
    let lower = added.to_ascii_lowercase();
    let markers = [
        (
            "filesystem",
            [
                "std::fs",
                "fs::",
                "write_text",
                "writefile",
                "pathbuf",
                "read_to_string",
            ]
            .as_slice(),
        ),
        (
            "network",
            [
                "reqwest",
                "httpx",
                "requests.",
                "fetch(",
                "websocket",
                "tcpstream",
            ]
            .as_slice(),
        ),
        (
            "process",
            [
                "std::process",
                "command::new",
                "subprocess",
                "popen",
                "powershell",
                "cmd.exe",
            ]
            .as_slice(),
        ),
        (
            "credentials",
            [
                "keyring",
                "credential",
                "api_key",
                "access_token",
                "private key",
                ".env",
            ]
            .as_slice(),
        ),
        (
            "windows-control",
            ["windows_sys", "shellexecute", "sendinput", "registry"].as_slice(),
        ),
    ];
    for (capability, needles) in markers {
        if needles.iter().any(|needle| lower.contains(needle)) {
            capabilities.insert(capability.to_string());
        }
    }
    if capabilities.contains("credentials") {
        security_findings.insert(
            "Accesso potenziale a credenziali o segreti: revisione critica obbligatoria."
                .to_string(),
        );
    }
    if capabilities.contains("process") {
        security_findings.insert(
            "Avvio potenziale di processi: consentito soltanto nel profilo sandbox dichiarato."
                .to_string(),
        );
    }
    if !dependency_changes.is_empty() {
        security_findings.insert(
            "Modifica delle dipendenze rilevata: bloccata dalla policy AURA corrente.".to_string(),
        );
    }
    if capabilities.is_empty() {
        capabilities.insert("ui-or-pure-computation".to_string());
    }
    let risk = if capabilities.contains("credentials") || capabilities.contains("process") {
        "critical"
    } else if capabilities.contains("network")
        || capabilities.contains("filesystem")
        || capabilities.contains("windows-control")
    {
        "moderate"
    } else {
        "safe"
    };
    Ok(EvolutionInspection {
        changed_files: changed_files.into_iter().collect(),
        capabilities: capabilities.into_iter().collect(),
        dependency_changes: dependency_changes.into_iter().collect(),
        security_findings: security_findings.into_iter().collect(),
        risk: risk.to_string(),
        patch_sha256: hex::encode(Sha256::digest(&bytes)),
        diff_excerpt: text.chars().take(24_000).collect(),
    })
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
            | "setup-evolution-sandbox.ps1"
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
    let _registration = super::task_control::register_child(child.id());
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
        canary_available: strong,
        automatic_rollback: true,
        attestation_version: 2,
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
        "--clearenv",
        "--setenv",
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        "--setenv",
        "HOME",
        "/tmp",
        "--setenv",
        "LANG",
        "C.UTF-8",
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
        decrypt_report, detect_status, encrypt_report, inspect_patch_file, sha256_file,
        strong_isolation, validate_text, verify_run_attestation, StoredEvolutionReport,
        StoredSandboxStatus,
    };
    use crate::crypto::Crypto;
    use std::fs;
    use tempfile::tempdir;
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
            inspection: None,
            diff_approved: false,
            attempts: 0,
            attestation: None,
            rollback_status: String::new(),
        };
        let crypto = Crypto::from_master([23_u8; 32]).expect("crypto");
        let encrypted = encrypt_report(&report, &crypto).expect("encrypt report");
        assert!(!String::from_utf8_lossy(&encrypted).contains(&report.objective));
        let view = decrypt_report(&encrypted, &id, &crypto).expect("decrypt report");
        assert_eq!(view.objective, report.objective);
        assert!(decrypt_report(&encrypted, &Uuid::new_v4().to_string(), &crypto).is_err());
    }

    #[test]
    fn patch_inspection_detects_capabilities_dependencies_and_secrets() {
        let directory = tempdir().expect("tempdir");
        let patch = directory.path().join("proposal.patch");
        fs::write(
            &patch,
            "diff --git a/package.json b/package.json\n+++ b/package.json\n+fetch(url)\n+const access_token = value\n",
        )
        .expect("patch");
        let inspection = inspect_patch_file(&patch).expect("inspection");
        assert_eq!(inspection.risk, "critical");
        assert!(inspection
            .changed_files
            .contains(&"package.json".to_string()));
        assert!(inspection.capabilities.contains(&"network".to_string()));
        assert!(inspection.capabilities.contains(&"credentials".to_string()));
        assert_eq!(inspection.dependency_changes, vec!["package.json"]);
        assert!(!inspection.patch_sha256.is_empty());
    }

    #[test]
    fn attestation_requires_fresh_offline_canary_and_approved_hash() {
        let directory = tempdir().expect("tempdir");
        let patch = directory.path().join("validated.patch");
        fs::write(
            &patch,
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-a\n+b\n",
        )
        .expect("patch");
        let hash = sha256_file(&patch).expect("hash");
        let run_id = Uuid::new_v4().to_string();
        let report = StoredEvolutionReport {
            version: 1,
            id: Uuid::new_v4().to_string(),
            project_name: "test".into(),
            objective: "test canary".into(),
            sandbox: StoredSandboxStatus {
                strong_isolation_available: true,
                execution_allowed: true,
                reason: "test".into(),
            },
            state: "diff-approved".into(),
            promotion: String::new(),
            created_at: "1".into(),
            artifact_path: String::new(),
            run_id: String::new(),
            inspection: Some(inspect_patch_file(&patch).expect("inspection")),
            diff_approved: true,
            attempts: 1,
            attestation: None,
            rollback_status: String::new(),
        };
        let attestation = serde_json::json!({
            "schemaVersion":2,"runId":run_id,"testProfile":"Static","patchSha256":hash,
            "dependencyPolicy":"immutable-offline","networkDuringTest":false,
            "hostFilesystemMountedReadOnly":true,"canaryPassed":true,
            "canaryFreshNamespace":true,"canaryNetworkDisabled":true,
            "changedFiles":["a.txt"],"securityFindings":[],"createdAtUtc":"2026-08-09T00:00:00Z"
        });
        fs::write(
            directory.path().join("attestation.json"),
            serde_json::to_vec(&attestation).unwrap(),
        )
        .unwrap();
        let canary = serde_json::json!({"schemaVersion":1,"runId":run_id,"passed":true,
            "freshNamespace":true,"networkDuringCanary":false,"hostFilesystemMountedReadOnly":true,"exitCode":0});
        fs::write(
            directory.path().join("canary.json"),
            serde_json::to_vec(&canary).unwrap(),
        )
        .unwrap();
        assert!(verify_run_attestation(directory.path(), &report, &run_id, "Static").is_ok());
        let mut tampered = canary;
        tampered["networkDuringCanary"] = serde_json::Value::Bool(true);
        fs::write(
            directory.path().join("canary.json"),
            serde_json::to_vec(&tampered).unwrap(),
        )
        .unwrap();
        assert!(verify_run_attestation(directory.path(), &report, &run_id, "Static").is_err());
    }
}
