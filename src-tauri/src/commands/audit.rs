use super::storage::{redact_db, AppState};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditStatus {
    valid: bool,
    count: usize,
    last_hash: String,
}

#[derive(Serialize)]
struct CanonicalAudit<'a> {
    previous_hash: &'a str,
    id: &'a str,
    correlation_id: &'a str,
    event_type: &'a str,
    decision: &'a str,
    summary: &'a str,
    payload_json: &'a str,
    created_at: &'a str,
}

pub(crate) fn append_audit(
    state: &AppState,
    correlation_id: &str,
    event_type: &str,
    decision: &str,
    summary: &str,
    payload_json: &str,
) -> Result<(), String> {
    if correlation_id.len() > 128 || event_type.len() > 64 || decision.len() > 32 || summary.len() > 512 || payload_json.len() > 16 * 1024 {
        return Err("Evento audit fuori dai limiti consentiti.".to_string());
    }
    serde_json::from_str::<serde_json::Value>(payload_json)
        .map_err(|_| "Payload audit non JSON.".to_string())?;
    let db = state.database.lock().map_err(|_| "Database occupato.".to_string())?;
    let previous_hash = db
        .query_row(
            "SELECT event_hash FROM audit_events ORDER BY sequence DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(redact_db)?
        .unwrap_or_default();
    let id = Uuid::new_v4().to_string();
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Orologio di sistema non valido.".to_string())?
        .as_millis()
        .to_string();
    let event_hash = calculate_hash(&CanonicalAudit {
        previous_hash: &previous_hash,
        id: &id,
        correlation_id,
        event_type,
        decision,
        summary,
        payload_json,
        created_at: &created_at,
    })?;
    let event_mac = state.crypto.hmac_hex(event_hash.as_bytes())?;
    db.execute(
        "INSERT INTO audit_events
         (id, correlation_id, event_type, decision, summary, payload_json, created_at, previous_hash, event_hash, event_mac)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, correlation_id, event_type, decision, summary, payload_json, created_at, previous_hash, event_hash, event_mac],
    )
    .map(|_| ())
    .map_err(redact_db)
}

#[tauri::command]
pub fn audit_verify(state: State<'_, AppState>) -> Result<AuditStatus, String> {
    verify_audit(&state)
}

fn verify_audit(state: &AppState) -> Result<AuditStatus, String> {
    let db = state.database.lock().map_err(|_| "Database occupato.".to_string())?;
    let mut statement = db
        .prepare(
            "SELECT id, correlation_id, event_type, COALESCE(decision,''), summary, payload_json,
                    created_at, previous_hash, event_hash, event_mac
             FROM audit_events ORDER BY sequence",
        )
        .map_err(redact_db)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?,
                row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(redact_db)?;

    let mut previous = String::new();
    let mut count = 0_usize;
    for row in rows {
        let (id, correlation_id, event_type, decision, summary, payload_json, created_at, previous_hash, event_hash, event_mac) = row.map_err(redact_db)?;
        let calculated = calculate_hash(&CanonicalAudit {
            previous_hash: &previous_hash, id: &id, correlation_id: &correlation_id,
            event_type: &event_type, decision: &decision, summary: &summary,
            payload_json: &payload_json, created_at: &created_at,
        })?;
        if previous_hash != previous || calculated != event_hash || !state.crypto.verify_hmac_hex(event_hash.as_bytes(), &event_mac) {
            return Ok(AuditStatus { valid: false, count, last_hash: previous });
        }
        previous = event_hash;
        count += 1;
    }
    Ok(AuditStatus { valid: true, count, last_hash: previous })
}

fn calculate_hash(event: &CanonicalAudit<'_>) -> Result<String, String> {
    let canonical = serde_json::to_vec(event).map_err(|_| "Serializzazione audit fallita.".to_string())?;
    Ok(hex::encode(Sha256::digest(canonical)))
}

#[cfg(test)]
mod tests {
    use super::{append_audit, verify_audit};
    use crate::{commands::storage::{harden_schema, AppState}, crypto::Crypto};
    use rusqlite::Connection;
    use std::sync::Mutex;

    fn state() -> AppState {
        let db = Connection::open_in_memory().expect("database");
        db.execute_batch(include_str!("../../migrations/0001_initial.sql")).expect("schema");
        harden_schema(&db).expect("harden schema");
        AppState { database: Mutex::new(db), crypto: Crypto::from_master([5_u8; 32]).expect("crypto") }
    }

    #[test]
    fn verifies_a_valid_hash_chain_and_detects_tampering() {
        let state = state();
        append_audit(&state, "c1", "policy.evaluated", "allow", "lettura", "{}").expect("first event");
        append_audit(&state, "c2", "policy.evaluated", "deny", "blocco", "{}").expect("second event");
        let valid = verify_audit(&state).expect("verify");
        assert!(valid.valid);
        assert_eq!(valid.count, 2);

        let db = state.database.lock().expect("database lock");
        db.execute_batch("DROP TRIGGER audit_events_no_update; UPDATE audit_events SET summary='alterato' WHERE sequence=1;")
            .expect("simulate tampering");
        drop(db);
        assert!(!verify_audit(&state).expect("verify tampered").valid);
    }
}
