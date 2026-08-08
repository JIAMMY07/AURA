use super::storage::{redact_db, AppState};
use crate::crypto::Crypto;
use rusqlite::{params, Connection, OptionalExtension};
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
    if correlation_id.len() > 128
        || event_type.len() > 64
        || decision.len() > 32
        || summary.len() > 512
        || payload_json.len() > 16 * 1024
    {
        return Err("Evento audit fuori dai limiti consentiti.".to_string());
    }
    serde_json::from_str::<serde_json::Value>(payload_json)
        .map_err(|_| "Payload audit non JSON.".to_string())?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
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
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
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
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(redact_db)?;

    let mut previous = String::new();
    let mut count = 0_usize;
    for row in rows {
        let (
            id,
            correlation_id,
            event_type,
            decision,
            summary,
            payload_json,
            created_at,
            previous_hash,
            event_hash,
            event_mac,
        ) = row.map_err(redact_db)?;
        let calculated = calculate_hash(&CanonicalAudit {
            previous_hash: &previous_hash,
            id: &id,
            correlation_id: &correlation_id,
            event_type: &event_type,
            decision: &decision,
            summary: &summary,
            payload_json: &payload_json,
            created_at: &created_at,
        })?;
        if previous_hash != previous
            || calculated != event_hash
            || !state
                .crypto
                .verify_hmac_hex(event_hash.as_bytes(), &event_mac)
        {
            return Ok(AuditStatus {
                valid: false,
                count,
                last_hash: previous,
            });
        }
        previous = event_hash;
        count += 1;
    }
    Ok(AuditStatus {
        valid: true,
        count,
        last_hash: previous,
    })
}

fn calculate_hash(event: &CanonicalAudit<'_>) -> Result<String, String> {
    let canonical =
        serde_json::to_vec(event).map_err(|_| "Serializzazione audit fallita.".to_string())?;
    Ok(hex::encode(Sha256::digest(canonical)))
}

pub(crate) fn migrate_legacy_audit(
    db: &mut Connection,
    crypto: &Crypto,
) -> Result<usize, Box<dyn std::error::Error>> {
    let needs_migration: i64 = db.query_row(
        "SELECT COUNT(*) FROM audit_events WHERE event_hash='' OR event_mac=''",
        [],
        |row| row.get(0),
    )?;
    if needs_migration == 0 {
        return Ok(0);
    }

    let rows = {
        let mut statement = db.prepare(
            "SELECT sequence,id,correlation_id,event_type,COALESCE(decision,''),summary,payload_json,created_at
             FROM audit_events ORDER BY sequence",
        )?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };

    let transaction = db.transaction()?;
    transaction.execute_batch(
        "DROP TRIGGER IF EXISTS audit_events_no_update;
         DROP TRIGGER IF EXISTS audit_events_no_delete;",
    )?;
    let mut previous_hash = String::new();
    for (sequence, id, correlation_id, event_type, decision, summary, payload_json, created_at) in
        &rows
    {
        let event_hash = calculate_hash(&CanonicalAudit {
            previous_hash: &previous_hash,
            id,
            correlation_id,
            event_type,
            decision,
            summary,
            payload_json,
            created_at,
        })
        .map_err(std::io::Error::other)?;
        let event_mac = crypto
            .hmac_hex(event_hash.as_bytes())
            .map_err(std::io::Error::other)?;
        transaction.execute(
            "UPDATE audit_events SET previous_hash=?1,event_hash=?2,event_mac=?3 WHERE sequence=?4",
            params![previous_hash, event_hash, event_mac, sequence],
        )?;
        previous_hash = event_hash;
    }
    transaction.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS audit_events_no_update BEFORE UPDATE ON audit_events
         BEGIN SELECT RAISE(ABORT, 'audit events are append-only'); END;
         CREATE TRIGGER IF NOT EXISTS audit_events_no_delete BEFORE DELETE ON audit_events
         BEGIN SELECT RAISE(ABORT, 'audit events are append-only'); END;",
    )?;
    transaction.commit()?;
    Ok(rows.len())
}

#[cfg(test)]
mod tests {
    use super::{append_audit, migrate_legacy_audit, verify_audit};
    use crate::{
        commands::storage::{harden_schema, AppState},
        crypto::Crypto,
    };
    use rusqlite::Connection;
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{atomic::AtomicBool, Arc, Mutex},
    };

    fn state() -> AppState {
        let db = Connection::open_in_memory().expect("database");
        db.execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .expect("schema");
        harden_schema(&db).expect("harden schema");
        AppState {
            database: Mutex::new(db),
            crypto: Crypto::from_master([5_u8; 32]).expect("crypto"),
            app_data: PathBuf::new(),
            cancellations: Mutex::new(HashMap::new()),
            generation_busy: Arc::new(AtomicBool::new(false)),
            voice_busy: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn verifies_a_valid_hash_chain_and_detects_tampering() {
        let state = state();
        append_audit(&state, "c1", "policy.evaluated", "allow", "lettura", "{}")
            .expect("first event");
        append_audit(&state, "c2", "policy.evaluated", "deny", "blocco", "{}")
            .expect("second event");
        let valid = verify_audit(&state).expect("verify");
        assert!(valid.valid);
        assert_eq!(valid.count, 2);

        let db = state.database.lock().expect("database lock");
        db.execute_batch("DROP TRIGGER audit_events_no_update; UPDATE audit_events SET summary='alterato' WHERE sequence=1;")
            .expect("simulate tampering");
        drop(db);
        assert!(!verify_audit(&state).expect("verify tampered").valid);
    }

    #[test]
    fn seals_legacy_audit_rows_once() {
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .expect("schema");
        super::super::storage::harden_schema(&db).expect("harden schema");
        db.execute(
            "INSERT INTO audit_events (id,correlation_id,event_type,decision,summary,payload_json,created_at)
             VALUES ('legacy','c1','legacy','allow','evento','{}','1')",
            [],
        )
        .expect("legacy event");
        let crypto = Crypto::from_master([11_u8; 32]).expect("crypto");
        assert_eq!(
            migrate_legacy_audit(&mut db, &crypto).expect("migration"),
            1
        );
        assert_eq!(
            migrate_legacy_audit(&mut db, &crypto).expect("second migration"),
            0
        );
        let state = AppState {
            database: Mutex::new(db),
            crypto,
            app_data: PathBuf::new(),
            cancellations: Mutex::new(HashMap::new()),
            generation_busy: Arc::new(AtomicBool::new(false)),
            voice_busy: Arc::new(AtomicBool::new(false)),
        };
        assert!(verify_audit(&state).expect("verify").valid);
    }
}
