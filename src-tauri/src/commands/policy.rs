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
    pub(crate) decision: &'static str,
    risk: &'static str,
    reason: &'static str,
}

pub(crate) fn enforce_capability(
    capability: &str,
    required_decision: &str,
) -> Result<PolicyDecision, String> {
    let decision = evaluate_capability(capability);
    if decision.decision != required_decision {
        return Err(format!(
            "Policy backend incoerente per {capability}: azione bloccata in sicurezza."
        ));
    }
    Ok(decision)
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
        "system.inspect"
        | "system.processes"
        | "profile.read"
        | "profile.feedback"
        | "voice.transcribe"
        | "voice.speak"
        | "browser.inspect"
        | "api.inspect"
        | "rag.read"
        | "3d.inspect"
        | "security.inspect"
        | "evolution.inspect"
        | "conversation.read"
        | "conversation.write"
        | "ai.chat" => PolicyDecision {
            decision: "allow",
            risk: "safe",
            reason: "Sola lettura nello scope locale consentito.",
        },
        "app.open"
        | "process.terminate"
        | "system.lock"
        | "profile.update"
        | "profile.clear"
        | "conversation.clear" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Consenso esplicito richiesto prima dell'azione locale.",
        },
        "browser.open"
        | "browser.navigate"
        | "api.configure"
        | "api.call"
        | "rag.import"
        | "rag.delete"
        | "3d.generate"
        | "3d.open" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Conferma esplicita richiesta per accesso esterno o modifica locale.",
        },
        "security.scan" | "security.update" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Azione difensiva Windows avviata soltanto dopo conferma esplicita.",
        },
        "evolution.propose" => PolicyDecision {
            decision: "sandbox",
            risk: "critical",
            reason: "La proposta puo essere registrata; esecuzione e promozione richiedono isolamento forte e revisione umana.",
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
    use super::{enforce_capability, evaluate_capability};

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

    #[test]
    fn pc_and_evolution_capabilities_have_fixed_risk() {
        assert_eq!(evaluate_capability("system.processes").decision, "allow");
        assert_eq!(evaluate_capability("app.open").decision, "confirm");
        assert_eq!(evaluate_capability("process.terminate").decision, "confirm");
        assert_eq!(evaluate_capability("system.lock").decision, "confirm");
        assert_eq!(evaluate_capability("evolution.propose").decision, "sandbox");
    }

    #[test]
    fn private_data_and_chat_capabilities_have_fixed_risk() {
        assert_eq!(evaluate_capability("profile.read").decision, "allow");
        assert_eq!(evaluate_capability("conversation.read").decision, "allow");
        assert_eq!(evaluate_capability("conversation.write").decision, "allow");
        assert_eq!(
            evaluate_capability("conversation.clear").decision,
            "confirm"
        );
        assert_eq!(evaluate_capability("ai.chat").decision, "allow");
        assert_eq!(evaluate_capability("rag.read").decision, "allow");
        assert_eq!(evaluate_capability("browser.inspect").decision, "allow");
        assert_eq!(evaluate_capability("api.call").decision, "confirm");
        assert_eq!(evaluate_capability("3d.generate").decision, "confirm");
    }

    #[test]
    fn tool_enforcement_fails_closed_if_policy_does_not_match() {
        assert!(enforce_capability("system.inspect", "allow").is_ok());
        assert!(enforce_capability("app.open", "allow").is_err());
        assert!(enforce_capability("unknown.tool", "allow").is_err());
    }
}
