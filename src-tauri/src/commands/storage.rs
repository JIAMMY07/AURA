use crate::crypto::Crypto;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, sync::Mutex};
use tauri::State;

const MAX_MESSAGES: usize = 100;
const MAX_CONTENT_BYTES: usize = 128 * 1024;

pub struct AppState {
    pub(crate) database: Mutex<Connection>,
    pub(crate) crypto: Crypto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    id: String,
    role: String,
    content: String,
    created_at: String,
}

impl AppState {
    pub fn open(app_data: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        fs::create_dir_all(app_data)?;
        let crypto = Crypto::load_or_create().map_err(std::io::Error::other)?;
        let mut database = Connection::open(app_data.join("aura.db"))?;
        database.execute_batch(include_str!("../../migrations/0001_initial.sql"))?;
        harden_schema(&database)?;
        migrate_plaintext_messages(&mut database, &crypto)?;
        Ok(Self { database: Mutex::new(database), crypto })
    }
}

#[tauri::command]
pub fn storage_load(state: State<'_, AppState>) -> Result<Vec<ChatMessage>, String> {
    let db = state.database.lock().map_err(|_| "Database occupato.".to_string())?;
    let mut statement = db
        .prepare(
            "SELECT id, role, content, created_at, content_cipher, content_nonce
             FROM messages ORDER BY sequence DESC LIMIT ?1",
        )
        .map_err(redact_db)?;
    let rows = statement
        .query_map([MAX_MESSAGES as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<Vec<u8>>>(4)?,
                row.get::<_, Option<Vec<u8>>>(5)?,
            ))
        })
        .map_err(redact_db)?;

    let mut messages = Vec::new();
    for row in rows {
        let (id, role, legacy_content, created_at, ciphertext, nonce) = row.map_err(redact_db)?;
        let content = match (ciphertext, nonce) {
            (Some(ciphertext), Some(nonce)) => state.crypto.decrypt(&ciphertext, &nonce)?,
            _ => legacy_content,
        };
        messages.push(ChatMessage { id, role, content, created_at });
    }
    messages.reverse();
    Ok(messages)
}

#[tauri::command]
pub fn storage_save(messages: Vec<ChatMessage>, state: State<'_, AppState>) -> Result<(), String> {
    let retained = messages.into_iter().rev().take(MAX_MESSAGES).collect::<Vec<_>>();
    for message in &retained { validate_message(message)?; }

    let mut db = state.database.lock().map_err(|_| "Database occupato.".to_string())?;
    let transaction = db.transaction().map_err(redact_db)?;
    for message in retained.iter().rev() {
        let (ciphertext, nonce) = state.crypto.encrypt(&message.content)?;
        transaction
            .execute(
                "INSERT INTO messages
                 (id, session_id, role, content, created_at, content_cipher, content_nonce, encryption_version)
                 VALUES (?1, 'default', ?2, '', ?3, ?4, ?5, 1)
                 ON CONFLICT(id) DO UPDATE SET
                   role=excluded.role, content='', created_at=excluded.created_at,
                   content_cipher=excluded.content_cipher, content_nonce=excluded.content_nonce,
                   encryption_version=1",
                params![message.id, message.role, message.created_at, ciphertext, nonce],
            )
            .map_err(redact_db)?;
    }
    transaction
        .execute(
            "DELETE FROM messages WHERE session_id='default' AND id NOT IN
             (SELECT id FROM messages WHERE session_id='default' ORDER BY sequence DESC LIMIT ?1)",
            [MAX_MESSAGES as i64],
        )
        .map_err(redact_db)?;
    transaction.commit().map_err(redact_db)
}

#[tauri::command]
pub fn storage_clear(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.database.lock().map_err(|_| "Database occupato.".to_string())?;
    db.execute("DELETE FROM messages WHERE session_id='default'", [])
        .map(|_| ())
        .map_err(redact_db)
}

fn validate_message(message: &ChatMessage) -> Result<(), String> {
    if message.id.is_empty() || message.id.len() > 128 {
        return Err("ID messaggio non valido.".to_string());
    }
    if !matches!(message.role.as_str(), "system" | "user" | "assistant") {
        return Err("Ruolo messaggio non valido.".to_string());
    }
    if message.content.len() > MAX_CONTENT_BYTES {
        return Err("Messaggio troppo grande.".to_string());
    }
    if message.created_at.len() > 64 {
        return Err("Data messaggio non valida.".to_string());
    }
    Ok(())
}

pub(crate) fn harden_schema(db: &Connection) -> rusqlite::Result<()> {
    add_column_if_missing(db, "messages", "content_cipher", "BLOB")?;
    add_column_if_missing(db, "messages", "content_nonce", "BLOB")?;
    add_column_if_missing(db, "messages", "encryption_version", "INTEGER NOT NULL DEFAULT 0")?;
    add_column_if_missing(db, "audit_events", "previous_hash", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(db, "audit_events", "event_hash", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(db, "audit_events", "event_mac", "TEXT NOT NULL DEFAULT ''")?;
    Ok(())
}

fn add_column_if_missing(db: &Connection, table: &str, column: &str, definition: &str) -> rusqlite::Result<()> {
    let exists = db
        .query_row(
            "SELECT 1 FROM pragma_table_info(?1) WHERE name=?2",
            params![table, column],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !exists {
        db.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"))?;
    }
    Ok(())
}

fn migrate_plaintext_messages(db: &mut Connection, crypto: &Crypto) -> Result<(), Box<dyn std::error::Error>> {
    let rows = {
        let mut statement = db.prepare(
            "SELECT id, content FROM messages WHERE content_cipher IS NULL OR encryption_version=0",
        )?;
        let collected = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let transaction = db.transaction()?;
    for (id, content) in rows {
        let (ciphertext, nonce) = crypto.encrypt(&content).map_err(std::io::Error::other)?;
        transaction.execute(
            "UPDATE messages SET content='', content_cipher=?1, content_nonce=?2, encryption_version=1 WHERE id=?3",
            params![ciphertext, nonce, id],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

pub(crate) fn redact_db(_: rusqlite::Error) -> String {
    "Operazione database locale fallita.".to_string()
}

#[cfg(test)]
mod tests {
    use super::{harden_schema, migrate_plaintext_messages};
    use crate::crypto::Crypto;
    use rusqlite::Connection;

    #[test]
    fn migrates_legacy_plaintext_to_authenticated_ciphertext() {
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch(include_str!("../../migrations/0001_initial.sql")).expect("schema");
        db.execute(
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES ('1','default','user','dato sensibile','now')",
            [],
        ).expect("legacy insert");
        harden_schema(&db).expect("harden schema");
        let crypto = Crypto::from_master([4_u8; 32]).expect("crypto");
        migrate_plaintext_messages(&mut db, &crypto).expect("migration");
        let (plain, cipher, nonce, version): (String, Vec<u8>, Vec<u8>, i64) = db.query_row(
            "SELECT content, content_cipher, content_nonce, encryption_version FROM messages WHERE id='1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).expect("migrated row");
        assert!(plain.is_empty());
        assert_eq!(version, 1);
        assert_eq!(crypto.decrypt(&cipher, &nonce).expect("decrypt"), "dato sensibile");
    }
}
