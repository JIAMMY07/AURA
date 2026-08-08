use super::{audit::append_audit, storage::AppState};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRequest {
    id: String,
    capability: String,
    summary: String,
    scope: String,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct PolicyDecision {
    decision: &'static str,
    risk: &'static str,
    reason: &'static str,
}

#[tauri::command]
pub fn policy_evaluate(
    action: ActionRequest,
    state: State<'_, AppState>,
) -> Result<PolicyDecision, String> {
    validate(&action)?;
    let result = evaluate_capability(&action.capability);
    let payload = serde_json::json!({
        "capability": action.capability,
        "risk": result.risk,
    });
    append_audit(
        &state,
        &action.id,
        "policy.evaluated",
        result.decision,
        "Valutazione capability backend",
        &payload.to_string(),
    )?;
    Ok(result)
}

pub(crate) fn evaluate_capability(capability: &str) -> PolicyDecision {
    match capability {
        "credential.export" | "security.disable" | "privilege.escalate" => PolicyDecision {
            decision: "deny",
            risk: "critical",
            reason: "Capability vietata dalla policy backend.",
        },
        "system.inspect" => PolicyDecision {
            decision: "allow",
            risk: "safe",
            reason: "Sola lettura nello scope locale consentito.",
        },
        "files.organize" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Conferma richiesta per modificare file.",
        },
        "module.install" => PolicyDecision {
            decision: "sandbox",
            risk: "critical",
            reason: "Sandbox e doppia conferma obbligatorie.",
        },
        _ => PolicyDecision {
            decision: "deny",
            risk: "critical",
            reason: "Capability non registrata: deny-by-default.",
        },
    }
}

fn validate(action: &ActionRequest) -> Result<(), String> {
    if action.id.is_empty()
        || action.id.len() > 128
        || action.capability.is_empty()
        || action.capability.len() > 128
    {
        return Err("Identificatore azione non valido.".to_string());
    }
    if action.summary.is_empty() || action.summary.len() > 512 || action.scope.len() > 512 {
        return Err("Descrizione azione fuori dai limiti.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::evaluate_capability;

    #[test]
    fn unknown_capabilities_are_denied() {
        assert_eq!(evaluate_capability("unknown.tool").decision, "deny");
    }

    #[test]
    fn caller_cannot_downgrade_registered_risk() {
        assert_eq!(evaluate_capability("module.install").decision, "sandbox");
    }

    #[test]
    fn file_changes_always_require_confirmation() {
        assert_eq!(evaluate_capability("files.organize").decision, "confirm");
    }
}
