use super::{audit::append_audit, storage::AppState};
use serde::Serialize;
use std::{env, fs, path::Path, process::Command};
use tauri::State;
use uuid::Uuid;

#[derive(Serialize)]
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

#[tauri::command]
pub fn sandbox_status() -> SandboxStatus {
    detect_status()
}

#[tauri::command]
pub fn evolution_record_proposal(
    project_name: String,
    objective: String,
    state: State<'_, AppState>,
) -> Result<EvolutionProposal, String> {
    validate_text(&project_name, 128, "Nome progetto")?;
    validate_text(&objective, 8_192, "Obiettivo")?;
    let status = detect_status();
    let id = Uuid::new_v4().to_string();
    let reports = state.app_data.join("evolution").join("reports");
    fs::create_dir_all(&reports)
        .map_err(|_| "Cartella report Evolution Lab non disponibile.".to_string())?;
    let report_path = reports.join(format!("{id}.json"));
    let report = serde_json::json!({
        "version": 1,
        "id": id,
        "projectName": project_name,
        "objective": objective,
        "sandbox": {
            "strongIsolationAvailable": status.strong_isolation_available,
            "executionAllowed": status.execution_allowed,
            "reason": status.reason,
        },
        "state": "proposal-only",
        "promotion": "manual-review-required"
    });
    fs::write(
        &report_path,
        serde_json::to_vec_pretty(&report)
            .map_err(|_| "Report Evolution Lab non serializzabile.".to_string())?,
    )
    .map_err(|_| "Scrittura report Evolution Lab fallita.".to_string())?;
    append_audit(
        &state,
        &id,
        "evolution.proposed",
        "sandbox",
        "Proposta registrata senza esecuzione",
        &serde_json::json!({ "projectName": project_name, "executable": false }).to_string(),
    )?;
    Ok(EvolutionProposal {
        id,
        report_path: report_path.to_string_lossy().to_string(),
        executable: false,
        status: "proposal-only".to_string(),
    })
}

fn detect_status() -> SandboxStatus {
    let windows_sandbox = Path::new(r"C:\Windows\System32\WindowsSandbox.exe").is_file();
    let wsl = Command::new("wsl.exe")
        .arg("--status")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    let container_runtime = executable_on_path("docker.exe") || executable_on_path("podman.exe");
    let strong = windows_sandbox || wsl || container_runtime;
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

#[cfg(test)]
mod tests {
    use super::validate_text;

    #[test]
    fn rejects_empty_or_oversized_proposals() {
        assert!(validate_text("", 10, "x").is_err());
        assert!(validate_text("troppo lungo", 3, "x").is_err());
        assert!(validate_text("ok", 10, "x").is_ok());
    }
}
