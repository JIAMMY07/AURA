use super::storage::{redact_db, AppState};
use crate::crypto::Crypto;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::State;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditStatus {
    pub(crate) valid: bool,
    pub(crate) count: usize,
    last_hash: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventView {
    sequence: i64,
    correlation_id: String,
    event_type: String,
    decision: String,
    summary: String,
    payload: Option<serde_json::Value>,
    created_at: String,
    event_hash: String,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionAuditInput {
    source: String,
    action: String,
    content: String,
    outcome: String,
    correlation_id: Option<String>,
    metadata: Option<serde_json::Value>,
}

fn interaction_aad(correlation_id: &str, source: &str, action: &str) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "kind": "interaction-log",
        "correlationId": correlation_id,
        "source": source,
        "action": action
    }))
    .map_err(|_| "Metadati del log non serializzabili.".to_string())
}

fn validate_interaction(
    source: &str,
    action: &str,
    content: &str,
    outcome: &str,
) -> Result<(), String> {
    if !matches!(source, "voice" | "alexa" | "gesture")
        || action.is_empty()
        || action.len() > 80
        || content.is_empty()
        || content.len() > 8_192
        || !matches!(outcome, "allow" | "confirm" | "deny" | "error" | "caution")
    {
        return Err("Interazione da registrare fuori dai limiti consentiti.".to_string());
    }
    Ok(())
}

pub(crate) fn append_interaction_audit(
    state: &AppState,
    correlation_id: &str,
    source: &str,
    action: &str,
    content: &str,
    outcome: &str,
    metadata: serde_json::Value,
) -> Result<(), String> {
    validate_interaction(source, action, content, outcome)?;
    if correlation_id.is_empty() || correlation_id.len() > 128 || !metadata.is_object() {
        return Err("Metadati dell'interazione non validi.".to_string());
    }
    let (ciphertext, nonce) = state
        .crypto
        .encrypt_with_aad(content, &interaction_aad(correlation_id, source, action)?)?;
    let payload = serde_json::json!({
        "interactionLogVersion": 1,
        "source": source,
        "action": action,
        "encryptedContent": {
            "ciphertext": STANDARD.encode(ciphertext),
            "nonce": STANDARD.encode(nonce)
        },
        "metadata": metadata,
        "audioStored": false,
        "imageStored": false,
        "secretLogged": false
    });
    append_audit(
        state,
        correlation_id,
        &format!("interaction.{source}"),
        outcome,
        &format!("Interazione {source}: {action}"),
        &payload.to_string(),
    )
}

#[tauri::command]
pub fn audit_record_interaction(
    input: InteractionAuditInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let correlation_id = input
        .correlation_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    append_interaction_audit(
        &state,
        &correlation_id,
        input.source.trim(),
        input.action.trim(),
        input.content.trim(),
        input.outcome.trim(),
        input.metadata.unwrap_or_else(|| serde_json::json!({})),
    )?;
    Ok(true)
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

#[tauri::command]
pub fn audit_list(
    limit: Option<usize>,
    event_type: Option<String>,
    decision: Option<String>,
    include_payloads: bool,
    state: State<'_, AppState>,
) -> Result<Vec<AuditEventView>, String> {
    super::policy::enforce_capability("audit.read", "allow")?;
    read_events(
        &state,
        limit.unwrap_or(200).clamp(1, 1_000),
        normalized_filter(event_type, 64)?,
        normalized_filter(decision, 32)?,
        include_payloads,
    )
}

#[tauri::command]
pub fn audit_export(include_payloads: bool, state: State<'_, AppState>) -> Result<String, String> {
    super::policy::enforce_capability("audit.read", "allow")?;
    let integrity = verify_audit(&state)?;
    let events = read_events(&state, 100_000, None, None, include_payloads)?;
    let exported_at = now_millis()?;
    let document = serde_json::json!({
        "schemaVersion": 1,
        "application": "AURA",
        "applicationVersion": env!("CARGO_PKG_VERSION"),
        "exportedAtUnixMs": exported_at,
        "integrity": {
            "valid": integrity.valid,
            "verifiedEvents": integrity.count,
            "lastHash": integrity.last_hash
        },
        "privacy": {
            "chatContentIncluded": false,
            "interactionContentIncluded": include_payloads,
            "payloadsIncluded": include_payloads,
            "notice": if include_payloads {
                "Le interazioni voce, Alexa e gesti sono decifrate nell'esportazione; possono inoltre essere presenti percorsi locali, URL e identificatori operativi."
            } else {
                "Esportazione diagnostica ridotta: chat, contenuti delle interazioni e payload tecnici esclusi."
            }
        },
        "events": events
    });
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|_| "Serializzazione dei log fallita.".to_string())?;
    let documents = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|path| path.join("Documents"))
        .unwrap_or_else(|| state.app_data.clone());
    let export_dir = documents.join("AURA").join("Log");
    fs::create_dir_all(&export_dir)
        .map_err(|_| "Impossibile creare la cartella dei log.".to_string())?;
    let path = export_dir.join(format!("AURA-log-{exported_at}.json"));
    fs::write(&path, bytes).map_err(|_| "Impossibile salvare i log.".to_string())?;
    if let Err(reason) = append_audit(
        &state,
        "audit-export",
        "audit.exported",
        "allow",
        "Log diagnostici esportati",
        &serde_json::json!({
            "payloadsIncluded": include_payloads,
            "exportedEvents": integrity.count,
            "interactionContentExported": include_payloads
        })
        .to_string(),
    ) {
        let _ = fs::remove_file(&path);
        return Err(reason);
    }
    Ok(path.to_string_lossy().into_owned())
}

fn normalized_filter(value: Option<String>, max_len: usize) -> Result<Option<String>, String> {
    let value = value.unwrap_or_default().trim().to_string();
    if value.len() > max_len {
        return Err("Filtro log fuori dai limiti consentiti.".to_string());
    }
    Ok((!value.is_empty()).then_some(value))
}

fn read_events(
    state: &AppState,
    limit: usize,
    event_type: Option<String>,
    decision: Option<String>,
    include_payloads: bool,
) -> Result<Vec<AuditEventView>, String> {
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let mut statement = db
        .prepare(
            "SELECT sequence,correlation_id,event_type,COALESCE(decision,''),summary,payload_json,created_at,event_hash
             FROM audit_events
             WHERE (?1 IS NULL OR event_type=?1) AND (?2 IS NULL OR COALESCE(decision,'')=?2)
             ORDER BY sequence DESC LIMIT ?3",
        )
        .map_err(redact_db)?;
    let rows = statement
        .query_map(
            params![event_type.as_deref(), decision.as_deref(), limit as i64],
            |row| {
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
            },
        )
        .map_err(redact_db)?;
    rows.map(|row| {
        let (
            sequence,
            correlation_id,
            event_type,
            decision,
            summary,
            payload_json,
            created_at,
            event_hash,
        ) = row.map_err(redact_db)?;
        let payload = if include_payloads {
            let mut value: serde_json::Value = serde_json::from_str(&payload_json)
                .map_err(|_| "Payload audit non valido.".to_string())?;
            if value
                .get("interactionLogVersion")
                .and_then(|item| item.as_u64())
                == Some(1)
            {
                let source = value
                    .get("source")
                    .and_then(|item| item.as_str())
                    .unwrap_or_default();
                let action = value
                    .get("action")
                    .and_then(|item| item.as_str())
                    .unwrap_or_default();
                let encrypted = value.get("encryptedContent").cloned().unwrap_or_default();
                let ciphertext = encrypted
                    .get("ciphertext")
                    .and_then(|item| item.as_str())
                    .unwrap_or_default();
                let nonce = encrypted
                    .get("nonce")
                    .and_then(|item| item.as_str())
                    .unwrap_or_default();
                let ciphertext = STANDARD
                    .decode(ciphertext)
                    .map_err(|_| "Contenuto log cifrato non valido.".to_string())?;
                let nonce = STANDARD
                    .decode(nonce)
                    .map_err(|_| "Nonce log cifrato non valido.".to_string())?;
                let content = state.crypto.decrypt_with_aad(
                    &ciphertext,
                    &nonce,
                    &interaction_aad(&correlation_id, source, action)?,
                )?;
                value["content"] = serde_json::Value::String(content);
                if let Some(object) = value.as_object_mut() {
                    object.remove("encryptedContent");
                }
            }
            Some(value)
        } else {
            None
        };
        Ok(AuditEventView {
            sequence,
            correlation_id,
            event_type,
            decision,
            summary,
            payload,
            created_at,
            event_hash,
        })
    })
    .collect()
}

fn now_millis() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| "Orologio di sistema non valido.".to_string())
}

pub(crate) fn verify_audit(state: &AppState) -> Result<AuditStatus, String> {
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
    use super::{
        append_audit, append_interaction_audit, migrate_legacy_audit, read_events, verify_audit,
    };
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
            tts_process: Mutex::new(None),
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
            tts_process: Mutex::new(None),
        };
        assert!(verify_audit(&state).expect("verify").valid);
    }

    #[test]
    fn diagnostic_listing_hides_payloads_unless_explicitly_requested() {
        let state = state();
        append_audit(
            &state,
            "request-1",
            "ai.generation.completed",
            "allow",
            "Generazione completata",
            r#"{"model":"local-model","contentLogged":false}"#,
        )
        .expect("audit event");

        let reduced = read_events(&state, 10, None, None, false).expect("reduced logs");
        assert_eq!(reduced.len(), 1);
        assert!(reduced[0].payload.is_none());

        let detailed = read_events(
            &state,
            10,
            Some("ai.generation.completed".to_string()),
            Some("allow".to_string()),
            true,
        )
        .expect("detailed logs");
        assert_eq!(detailed.len(), 1);
        assert_eq!(
            detailed[0]
                .payload
                .as_ref()
                .and_then(|value| value["model"].as_str()),
            Some("local-model")
        );
    }

    #[test]
    fn interaction_content_is_encrypted_and_only_decrypted_on_request() {
        let state = state();
        let secret_content = "apri il browser e cerca documentazione Rust";
        append_interaction_audit(
            &state,
            "voice-1",
            "voice",
            "chat-submit",
            secret_content,
            "allow",
            serde_json::json!({"confidence": 91, "audioStored": false}),
        )
        .expect("interaction audit");

        let raw_payload: String = state
            .database
            .lock()
            .expect("database")
            .query_row("SELECT payload_json FROM audit_events LIMIT 1", [], |row| {
                row.get(0)
            })
            .expect("payload");
        assert!(!raw_payload.contains(secret_content));

        let reduced = read_events(&state, 10, None, None, false).expect("reduced");
        assert!(reduced[0].payload.is_none());
        let detailed = read_events(&state, 10, None, None, true).expect("detailed");
        assert_eq!(
            detailed[0]
                .payload
                .as_ref()
                .and_then(|value| value["content"].as_str()),
            Some(secret_content)
        );
        assert!(verify_audit(&state).expect("integrity").valid);
    }
}
