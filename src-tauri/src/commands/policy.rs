use super::{audit::append_audit, storage::AppState};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::State;

const ACCESS_READ_ONLY: u8 = 0;
const ACCESS_CONTROLLED: u8 = 1;
const ACCESS_SANDBOX_AUTO: u8 = 2;
static ACCESS_MODE: AtomicU8 = AtomicU8::new(ACCESS_CONTROLLED);

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcAccessStatus {
    mode: &'static str,
    label: &'static str,
    sandbox_scope: &'static str,
    network: &'static str,
    approvals: &'static str,
    warning: &'static str,
}

pub(crate) fn enforce_capability(
    capability: &str,
    required_decision: &str,
) -> Result<PolicyDecision, String> {
    let decision = evaluate_capability(capability);
    if ACCESS_MODE.load(Ordering::SeqCst) == ACCESS_READ_ONLY
        && read_only_blocks(capability, decision.decision)
    {
        return Err(format!(
            "Accesso PC in sola lettura: capability {capability} bloccata. Cambia modalità dal banner Accesso PC."
        ));
    }
    if decision.decision != required_decision {
        return Err(format!(
            "Policy backend incoerente per {capability}: azione bloccata in sicurezza."
        ));
    }
    Ok(decision)
}

#[tauri::command]
pub fn policy_get_access_mode() -> PcAccessStatus {
    access_status(ACCESS_MODE.load(Ordering::SeqCst))
}

#[tauri::command]
pub fn policy_set_access_mode(
    mode: String,
    state: State<'_, AppState>,
) -> Result<PcAccessStatus, String> {
    let requested = match mode.as_str() {
        "read-only" => ACCESS_READ_ONLY,
        "controlled" => ACCESS_CONTROLLED,
        "sandbox-auto" => ACCESS_SANDBOX_AUTO,
        _ => return Err("Profilo di accesso PC non valido.".to_string()),
    };
    let current = ACCESS_MODE.load(Ordering::SeqCst);
    if requested > current {
        let message = if requested == ACCESS_SANDBOX_AUTO {
            "Abilitare l'autonomia confinata? Browser multi-step ed Evolution potranno procedere automaticamente dentro i limiti tipizzati e la sandbox. Modifiche host, promozioni, credenziali e azioni distruttive continueranno a richiedere conferma o resteranno vietate."
        } else {
            "Passare dalla sola lettura alla modalità controllata? Le azioni con effetti richiederanno comunque conferma nativa."
        };
        if !super::pc::confirm_native_action("Modifica accesso PC AURA", message) {
            return Ok(access_status(current));
        }
    }
    ACCESS_MODE.store(requested, Ordering::SeqCst);
    let status = access_status(requested);
    append_audit(
        &state,
        "pc-access-mode",
        "policy.access_mode_changed",
        if requested > current {
            "confirm"
        } else {
            "allow"
        },
        "Profilo accesso PC modificato",
        &serde_json::json!({"mode":status.mode,"contentLogged":false}).to_string(),
    )?;
    Ok(status)
}

fn access_status(mode: u8) -> PcAccessStatus {
    match mode {
        ACCESS_READ_ONLY => PcAccessStatus {
            mode: "read-only",
            label: "Sola lettura",
            sandbox_scope: "Diagnostica e consultazione; modifiche e automazioni bloccate",
            network: "Nessuna nuova sessione browser o chiamata con effetti",
            approvals: "Il profilo deve essere elevato manualmente",
            warning: "Chat e archivio locale continuano a essere salvati cifrati.",
        },
        ACCESS_SANDBOX_AUTO => PcAccessStatus {
            mode: "sandbox-auto",
            label: "Autonomia in sandbox",
            sandbox_scope: "Browser multi-step ed Evolution autonomi entro confini verificati",
            network: "Solo HTTPS pubblico con SSRF/DNS policy; sandbox Evolution offline",
            approvals: "Conferme obbligatorie per host, diff, promozione e azioni distruttive",
            warning: "Non equivale ad accesso amministratore illimitato.",
        },
        _ => PcAccessStatus {
            mode: "controlled",
            label: "Controllato",
            sandbox_scope: "Letture automatiche; strumenti con effetti tramite conferma",
            network: "Browser/API tipizzati con policy di destinazione",
            approvals: "Utente revisore per azioni moderate o critiche",
            warning: "Modalità predefinita consigliata.",
        },
    }
}

fn read_only_blocks(capability: &str, base_decision: &str) -> bool {
    base_decision != "allow" || matches!(capability, "media.control")
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
        | "app.inspect"
        | "media.control"
        | "profile.read"
        | "profile.feedback"
        | "voice.transcribe"
        | "voice.speak"
        | "browser.inspect"
        | "screen.inspect"
        | "api.inspect"
        | "rag.read"
        | "3d.inspect"
        | "security.inspect"
        | "security.incident.inspect"
        | "privacy.inspect"
        | "audit.read"
        | "remote.inspect"
        | "remote.poll"
        | "evolution.inspect"
        | "conversation.read"
        | "conversation.write"
        | "workspace.inspect"
        | "workspace.read"
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
        | "conversation.clear"
        | "workspace.grant"
        | "workspace.write"
        | "workspace.move"
        | "workspace.delete"
        | "workspace.open" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Consenso esplicito richiesto prima dell'azione locale.",
        },
        "desktop.gesture.control" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Il controllo gestuale del desktop richiede un'abilitazione locale esplicita e temporanea.",
        },
        "window.control" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Il controllo della finestra attiva richiede un intento locale esplicito; la chiusura richiede conferma nativa.",
        },
        "evolution.approve" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Il diff effettivo richiede approvazione esplicita prima dei test.",
        },
        "browser.open"
        | "browser.navigate"
        | "browser.act"
        | "browser.agent"
        | "browser.download"
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
        "security.scan" | "security.update" | "security.remediate" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Azione difensiva Windows avviata soltanto dopo conferma esplicita.",
        },
        "remote.pair" | "remote.unpair" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Associazione o revoca remota consentita soltanto dopo conferma locale.",
        },
        "remote.command" => PolicyDecision {
            decision: "confirm",
            risk: "moderate",
            reason: "Un comando remoto firmato non sostituisce la policy dell'azione richiesta.",
        },
        "evolution.propose" => PolicyDecision {
            decision: "sandbox",
            risk: "critical",
            reason: "La proposta puo essere registrata; esecuzione e promozione richiedono isolamento forte e revisione umana.",
        },
        "sandbox.install" => PolicyDecision {
            decision: "confirm",
            risk: "critical",
            reason: "Installazione di componenti Windows soltanto con conferma locale e UAC.",
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
    use super::{
        access_status, evaluate_capability, read_only_blocks, ACCESS_READ_ONLY, ACCESS_SANDBOX_AUTO,
    };

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
        assert_eq!(evaluate_capability("app.inspect").decision, "allow");
        assert_eq!(evaluate_capability("process.terminate").decision, "confirm");
        assert_eq!(evaluate_capability("system.lock").decision, "confirm");
        assert_eq!(evaluate_capability("workspace.read").decision, "allow");
        assert_eq!(evaluate_capability("workspace.write").decision, "confirm");
        assert_eq!(evaluate_capability("workspace.delete").decision, "confirm");
        assert_eq!(evaluate_capability("evolution.propose").decision, "sandbox");
    }

    #[test]
    fn private_data_and_chat_capabilities_have_fixed_risk() {
        assert_eq!(evaluate_capability("profile.read").decision, "allow");
        assert_eq!(evaluate_capability("conversation.read").decision, "allow");
        assert_eq!(evaluate_capability("audit.read").decision, "allow");
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
    fn incident_response_allows_triage_but_confirms_remediation() {
        assert_eq!(
            evaluate_capability("security.incident.inspect").decision,
            "allow"
        );
        assert_eq!(
            evaluate_capability("security.remediate").decision,
            "confirm"
        );
        assert_eq!(evaluate_capability("security.hackback").decision, "deny");
    }

    #[test]
    fn tool_enforcement_fails_closed_if_policy_does_not_match() {
        assert!(!read_only_blocks("system.inspect", "allow"));
        assert!(read_only_blocks("media.control", "allow"));
        assert!(read_only_blocks("app.open", "confirm"));
        assert_eq!(access_status(ACCESS_READ_ONLY).mode, "read-only");
        assert_eq!(access_status(ACCESS_SANDBOX_AUTO).mode, "sandbox-auto");
    }
}
