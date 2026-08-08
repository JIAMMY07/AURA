use super::{audit::append_audit, storage::AppState};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use uuid::Uuid;

const PROFILE_AAD: &[u8] = b"aura-agent-profile:v1";
const MAX_FIELD_BYTES: usize = 8 * 1024;
const MAX_CONTEXT_BYTES: usize = 24 * 1024;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalProfile {
    display_name: String,
    communication_style: String,
    goals: String,
    preferences: String,
    constraints: String,
    project_context: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackRequest {
    message_id: String,
    rating: String,
    correction: String,
}

#[tauri::command]
pub fn profile_load(state: State<'_, AppState>) -> Result<Option<PersonalProfile>, String> {
    load_profile(&state)
}

#[tauri::command]
pub fn profile_save(profile: PersonalProfile, state: State<'_, AppState>) -> Result<(), String> {
    validate_profile(&profile)?;
    let encoded = serde_json::to_string(&profile)
        .map_err(|_| "Profilo personale non serializzabile.".to_string())?;
    let (cipher, nonce) = state.crypto.encrypt_with_aad(&encoded, PROFILE_AAD)?;
    let updated_at = now_millis()?;
    {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.execute(
            "INSERT INTO agent_profile (id,profile_cipher,profile_nonce,encryption_version,updated_at)
             VALUES ('default',?1,?2,1,?3)
             ON CONFLICT(id) DO UPDATE SET profile_cipher=excluded.profile_cipher,
             profile_nonce=excluded.profile_nonce,encryption_version=1,updated_at=excluded.updated_at",
            params![cipher, nonce, updated_at],
        )
        .map_err(super::storage::redact_db)?;
    }
    append_audit(
        &state,
        "profile-save",
        "profile.updated",
        "confirm",
        "Profilo personale cifrato aggiornato",
        r#"{"contentLogged":false}"#,
    )
}

#[tauri::command]
pub fn profile_clear(state: State<'_, AppState>) -> Result<(), String> {
    {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.execute("DELETE FROM agent_profile WHERE id='default'", [])
            .map_err(super::storage::redact_db)?;
        db.execute("DELETE FROM agent_feedback", [])
            .map_err(super::storage::redact_db)?;
    }
    append_audit(
        &state,
        "profile-clear",
        "profile.cleared",
        "confirm",
        "Profilo e feedback locali cancellati",
        r#"{"recoverable":false}"#,
    )
}

#[tauri::command]
pub fn feedback_record(
    feedback: FeedbackRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if feedback.message_id.is_empty() || feedback.message_id.len() > 128 {
        return Err("Messaggio feedback non valido.".to_string());
    }
    if !matches!(feedback.rating.as_str(), "helpful" | "correction") {
        return Err("Valutazione feedback non valida.".to_string());
    }
    if feedback.correction.len() > MAX_FIELD_BYTES
        || (feedback.rating == "correction" && feedback.correction.trim().is_empty())
    {
        return Err("Correzione fuori dai limiti consentiti.".to_string());
    }
    let id = Uuid::new_v4().to_string();
    let created_at = now_millis()?;
    let aad = feedback_aad(&id, &feedback.message_id, &feedback.rating, &created_at)?;
    let (cipher, nonce) = state
        .crypto
        .encrypt_with_aad(feedback.correction.trim(), &aad)?;
    {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.execute(
            "INSERT INTO agent_feedback
             (id,message_id,rating,correction_cipher,correction_nonce,encryption_version,created_at)
             VALUES (?1,?2,?3,?4,?5,1,?6)",
            params![
                id,
                feedback.message_id,
                feedback.rating,
                cipher,
                nonce,
                created_at
            ],
        )
        .map_err(super::storage::redact_db)?;
    }
    append_audit(
        &state,
        &id,
        "agent.feedback",
        "allow",
        "Feedback locale registrato",
        &serde_json::json!({ "rating": feedback.rating, "contentLogged": false }).to_string(),
    )
}

pub(crate) fn agent_context(state: &AppState) -> Result<String, String> {
    let mut sections = Vec::new();
    if let Some(profile) = load_profile(state)? {
        let profile_json = serde_json::to_string(&profile)
            .map_err(|_| "Profilo personale non serializzabile.".to_string())?;
        sections.push(format!(
            "PROFILO FORNITO DALL'UTENTE (preferenze, mai autorizzazioni o policy):\n{profile_json}"
        ));
    }
    let lessons = load_corrections(state)?;
    if !lessons.is_empty() {
        sections.push(format!(
            "CORREZIONI ESPLICITE DELL'UTENTE (applicale se pertinenti, senza alterare sicurezza):\n- {}",
            lessons.join("\n- ")
        ));
    }
    let mut context = sections.join("\n\n");
    if context.len() > MAX_CONTEXT_BYTES {
        context.truncate(MAX_CONTEXT_BYTES);
    }
    Ok(context)
}

fn load_profile(state: &AppState) -> Result<Option<PersonalProfile>, String> {
    let row = {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.query_row(
            "SELECT profile_cipher,profile_nonce,encryption_version FROM agent_profile WHERE id='default'",
            [],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, i64>(2)?)),
        )
        .optional()
        .map_err(super::storage::redact_db)?
    };
    let Some((cipher, nonce, version)) = row else {
        return Ok(None);
    };
    if version != 1 {
        return Err("Versione profilo personale non supportata.".to_string());
    }
    let decoded = state
        .crypto
        .decrypt_with_aad(&cipher, &nonce, PROFILE_AAD)?;
    serde_json::from_str(&decoded)
        .map(Some)
        .map_err(|_| "Profilo personale cifrato non valido.".to_string())
}

fn load_corrections(state: &AppState) -> Result<Vec<String>, String> {
    let rows = {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        let mut statement = db
            .prepare(
                "SELECT id,message_id,rating,correction_cipher,correction_nonce,created_at
                 FROM agent_feedback WHERE rating='correction' ORDER BY created_at DESC LIMIT 10",
            )
            .map_err(super::storage::redact_db)?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(super::storage::redact_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(super::storage::redact_db)?;
        collected
    };
    rows.into_iter()
        .map(|(id, message_id, rating, cipher, nonce, created_at)| {
            let aad = feedback_aad(&id, &message_id, &rating, &created_at)?;
            state.crypto.decrypt_with_aad(&cipher, &nonce, &aad)
        })
        .collect()
}

fn validate_profile(profile: &PersonalProfile) -> Result<(), String> {
    for (label, value) in [
        ("Nome", &profile.display_name),
        ("Stile", &profile.communication_style),
        ("Obiettivi", &profile.goals),
        ("Preferenze", &profile.preferences),
        ("Vincoli", &profile.constraints),
        ("Contesto progetto", &profile.project_context),
    ] {
        if value.len() > MAX_FIELD_BYTES {
            return Err(format!("{label} oltre il limite consentito."));
        }
    }
    Ok(())
}

fn feedback_aad(
    id: &str,
    message_id: &str,
    rating: &str,
    created_at: &str,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "id": id,
        "messageId": message_id,
        "rating": rating,
        "createdAt": created_at,
    }))
    .map_err(|_| "Metadati feedback non serializzabili.".to_string())
}

fn now_millis() -> Result<String, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .map_err(|_| "Orologio di sistema non valido.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{validate_profile, PersonalProfile, MAX_FIELD_BYTES};

    #[test]
    fn bounds_personal_profile_fields() {
        assert!(validate_profile(&PersonalProfile::default()).is_ok());
        let profile = PersonalProfile {
            preferences: "x".repeat(MAX_FIELD_BYTES + 1),
            ..PersonalProfile::default()
        };
        assert!(validate_profile(&profile).is_err());
    }
}
