use super::{audit::append_audit, policy::enforce_capability, storage::AppState};
use crate::crypto::Crypto;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::Path,
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
    })
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
    let strong = strong_isolation(windows_sandbox, wsl, container_runtime);
    SandboxStatus {
        strong_isolation_available: strong,
        windows_sandbox,
        wsl,
        container_runtime,
        simulation_available: true,
        execution_allowed: false,
        reason: if strong {
            "Runtime rilevato, ma l'esecuzione resta disabilitata finché immagini, rete e policy non sono verificate."
        } else {
            "Nessuna sandbox forte rilevata: consentite solo proposta, diff e revisione manuale."
        }
        .to_string(),
    }
}

fn strong_isolation(windows_sandbox: bool, wsl: bool, container_runtime: bool) -> bool {
    windows_sandbox || (wsl && container_runtime)
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
    fn execution_remains_fail_closed_for_every_detected_runtime() {
        assert!(!detect_status().execution_allowed);
        assert!(strong_isolation(true, false, false));
        assert!(strong_isolation(false, true, true));
        assert!(!strong_isolation(false, true, false));
        assert!(!strong_isolation(false, false, true));
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
        };
        let crypto = Crypto::from_master([23_u8; 32]).expect("crypto");
        let encrypted = encrypt_report(&report, &crypto).expect("encrypt report");
        assert!(!String::from_utf8_lossy(&encrypted).contains(&report.objective));
        let view = decrypt_report(&encrypted, &id, &crypto).expect("decrypt report");
        assert_eq!(view.objective, report.objective);
        assert!(decrypt_report(&encrypted, &Uuid::new_v4().to_string(), &crypto).is_err());
    }
}
