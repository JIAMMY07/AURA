use crate::crypto::Crypto;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Child,
    sync::{atomic::AtomicBool, Arc, Mutex},
};
use tauri::State;

const MAX_MESSAGES: usize = 100;
const MAX_CONTENT_BYTES: usize = 128 * 1024;

pub struct AppState {
    pub(crate) database: Mutex<Connection>,
    pub(crate) crypto: Crypto,
    pub(crate) app_data: PathBuf,
    pub(crate) cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub(crate) generation_busy: Arc<AtomicBool>,
    pub(crate) voice_busy: Arc<AtomicBool>,
    pub(crate) tts_process: Mutex<Option<Child>>,
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
        configure_database(&database)?;
        database.execute_batch(include_str!("../../migrations/0001_initial.sql"))?;
        harden_schema(&database)?;
        let migrated_messages = migrate_messages_to_v2(&mut database, &crypto)?;
        let migrated_audit = super::audit::migrate_legacy_audit(&mut database, &crypto)?;
        database.pragma_update(None, "user_version", 2)?;
        if migrated_messages > 0 || migrated_audit > 0 {
            database.execute_batch(
                "PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA wal_checkpoint(TRUNCATE);",
            )?;
        }
        Ok(Self {
            database: Mutex::new(database),
            crypto,
            app_data: app_data.to_path_buf(),
            cancellations: Mutex::new(HashMap::new()),
            generation_busy: Arc::new(AtomicBool::new(false)),
            voice_busy: Arc::new(AtomicBool::new(false)),
            tts_process: Mutex::new(None),
        })
    }
}

#[tauri::command]
pub fn storage_load(state: State<'_, AppState>) -> Result<Vec<ChatMessage>, String> {
    super::policy::enforce_capability("conversation.read", "allow")?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let mut statement = db
        .prepare(
            "SELECT id, role, content, created_at, content_cipher, content_nonce, encryption_version
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
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(redact_db)?;

    let mut messages = Vec::new();
    for row in rows {
        let (id, role, legacy_content, created_at, ciphertext, nonce, version) =
            row.map_err(redact_db)?;
        let content = match (version, ciphertext, nonce) {
            (2, Some(ciphertext), Some(nonce)) => state.crypto.decrypt_with_aad(
                &ciphertext,
                &nonce,
                &message_aad(&id, &role, &created_at)?,
            )?,
            (1, Some(ciphertext), Some(nonce)) => state.crypto.decrypt(&ciphertext, &nonce)?,
            (0, _, _) => legacy_content,
            _ => return Err("Versione cifratura messaggio non supportata.".to_string()),
        };
        messages.push(ChatMessage {
            id,
            role,
            content,
            created_at,
        });
    }
    messages.reverse();
    Ok(messages)
}

#[tauri::command]
pub fn storage_save(messages: Vec<ChatMessage>, state: State<'_, AppState>) -> Result<(), String> {
    super::policy::enforce_capability("conversation.write", "allow")?;
    let retained = messages
        .into_iter()
        .rev()
        .take(MAX_MESSAGES)
        .collect::<Vec<_>>();
    for message in &retained {
        validate_message(message)?;
    }

    let mut db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let transaction = db.transaction().map_err(redact_db)?;
    transaction
        .execute("DELETE FROM messages WHERE session_id='default'", [])
        .map_err(redact_db)?;
    for message in retained.iter().rev() {
        let aad = message_aad(&message.id, &message.role, &message.created_at)?;
        let (ciphertext, nonce) = state.crypto.encrypt_with_aad(&message.content, &aad)?;
        transaction
            .execute(
                "INSERT INTO messages
                 (id, session_id, role, content, created_at, content_cipher, content_nonce, encryption_version)
                 VALUES (?1, 'default', ?2, '', ?3, ?4, ?5, 2)",
                params![message.id, message.role, message.created_at, ciphertext, nonce],
            )
            .map_err(redact_db)?;
    }
    transaction
        .execute(
            "UPDATE sessions SET updated_at=datetime('now') WHERE id='default'",
            [],
        )
        .map_err(redact_db)?;
    transaction.commit().map_err(redact_db)
}

#[tauri::command]
pub fn storage_clear(state: State<'_, AppState>) -> Result<bool, String> {
    super::policy::enforce_capability("conversation.clear", "confirm")?;
    if !super::pc::confirm_native_action(
        "Conferma cancellazione",
        "Cancellare definitivamente la cronologia locale cifrata? L'azione non puo essere annullata.",
    ) {
        super::audit::append_audit(
            &state,
            "storage-clear",
            "storage.clear_cancelled",
            "confirm",
            "Cancellazione cronologia annullata dall'utente",
            r#"{"contentLogged":false}"#,
        )?;
        return Ok(false);
    }
    {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.execute("DELETE FROM messages WHERE session_id='default'", [])
            .map_err(redact_db)?;
    }
    super::audit::append_audit(
        &state,
        "storage-clear",
        "storage.cleared",
        "confirm",
        "Cronologia locale cancellata",
        r#"{"contentLogged":false}"#,
    )?;
    Ok(true)
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
    add_column_if_missing(
        db,
        "messages",
        "encryption_version",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        db,
        "audit_events",
        "previous_hash",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(db, "audit_events", "event_hash", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(db, "audit_events", "event_mac", "TEXT NOT NULL DEFAULT ''")?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_profile (
            id TEXT PRIMARY KEY CHECK(id='default'),
            profile_cipher BLOB NOT NULL,
            profile_nonce BLOB NOT NULL,
            encryption_version INTEGER NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS agent_feedback (
            id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL,
            rating TEXT NOT NULL CHECK(rating IN ('helpful','correction')),
            correction_cipher BLOB NOT NULL,
            correction_nonce BLOB NOT NULL,
            encryption_version INTEGER NOT NULL,
            created_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_agent_feedback_created ON agent_feedback(created_at DESC);",
    )?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS api_connectors (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            auth_kind TEXT NOT NULL,
            auth_header TEXT NOT NULL,
            secret_cipher BLOB NOT NULL,
            secret_nonce BLOB NOT NULL,
            created_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS rag_documents (
            id TEXT PRIMARY KEY,
            source_name TEXT NOT NULL,
            source_cipher BLOB NOT NULL,
            source_nonce BLOB NOT NULL,
            chunk_count INTEGER NOT NULL,
            created_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS rag_chunks (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL REFERENCES rag_documents(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            content_cipher BLOB NOT NULL,
            content_nonce BLOB NOT NULL,
            embedding_cipher BLOB NOT NULL,
            embedding_nonce BLOB NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_rag_chunks_document ON rag_chunks(document_id, ordinal);",
    )?;
    Ok(())
}

fn configure_database(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "PRAGMA foreign_keys=ON;
         PRAGMA journal_mode=WAL;
         PRAGMA synchronous=FULL;
         PRAGMA busy_timeout=5000;
         PRAGMA secure_delete=ON;",
    )
}

fn add_column_if_missing(
    db: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let exists = db
        .query_row(
            "SELECT 1 FROM pragma_table_info(?1) WHERE name=?2",
            params![table, column],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !exists {
        db.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))?;
    }
    Ok(())
}

fn migrate_messages_to_v2(
    db: &mut Connection,
    crypto: &Crypto,
) -> Result<usize, Box<dyn std::error::Error>> {
    let rows = {
        let mut statement = db.prepare(
            "SELECT id, role, content, created_at, content_cipher, content_nonce, encryption_version
             FROM messages WHERE encryption_version<>2 OR content_cipher IS NULL",
        )?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<Vec<u8>>>(4)?,
                    row.get::<_, Option<Vec<u8>>>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let count = rows.len();
    let transaction = db.transaction()?;
    for (id, role, legacy_content, created_at, old_cipher, old_nonce, version) in rows {
        let content = match (version, old_cipher, old_nonce) {
            (1, Some(cipher), Some(nonce)) => crypto
                .decrypt(&cipher, &nonce)
                .map_err(std::io::Error::other)?,
            (0, _, _) => legacy_content,
            _ => return Err("Stato cifratura legacy non valido.".into()),
        };
        let aad = message_aad(&id, &role, &created_at).map_err(std::io::Error::other)?;
        let (ciphertext, nonce) = crypto
            .encrypt_with_aad(&content, &aad)
            .map_err(std::io::Error::other)?;
        transaction.execute(
            "UPDATE messages SET content='', content_cipher=?1, content_nonce=?2, encryption_version=2 WHERE id=?3",
            params![ciphertext, nonce, id],
        )?;
    }
    transaction.commit()?;
    Ok(count)
}

fn message_aad(id: &str, role: &str, created_at: &str) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&serde_json::json!({
        "version": 2,
        "sessionId": "default",
        "messageId": id,
        "role": role,
        "createdAt": created_at,
    }))
    .map_err(|_| "Metadati messaggio non serializzabili.".to_string())
}

pub(crate) fn redact_db(_: rusqlite::Error) -> String {
    "Operazione database locale fallita.".to_string()
}

#[cfg(test)]
mod tests {
    use super::{configure_database, harden_schema, message_aad, migrate_messages_to_v2};
    use crate::crypto::Crypto;
    use rusqlite::{params, Connection};

    #[test]
    fn enables_secure_database_pragmas() {
        let db = Connection::open_in_memory().expect("database");
        configure_database(&db).expect("configuration");
        let secure_delete: i64 = db
            .pragma_query_value(None, "secure_delete", |row| row.get(0))
            .expect("secure delete");
        let foreign_keys: i64 = db
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .expect("foreign keys");
        assert_eq!(secure_delete, 1);
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn migrates_legacy_plaintext_to_authenticated_ciphertext() {
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .expect("schema");
        db.execute(
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES ('1','default','user','dato sensibile','now')",
            [],
        ).expect("legacy insert");
        harden_schema(&db).expect("harden schema");
        let crypto = Crypto::from_master([4_u8; 32]).expect("crypto");
        migrate_messages_to_v2(&mut db, &crypto).expect("migration");
        let (plain, cipher, nonce, version): (String, Vec<u8>, Vec<u8>, i64) = db.query_row(
            "SELECT content, content_cipher, content_nonce, encryption_version FROM messages WHERE id='1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).expect("migrated row");
        assert!(plain.is_empty());
        assert_eq!(version, 2);
        let aad = message_aad("1", "user", "now").expect("aad");
        assert_eq!(
            crypto
                .decrypt_with_aad(&cipher, &nonce, &aad)
                .expect("decrypt"),
            "dato sensibile"
        );
    }

    #[test]
    fn migrates_v1_ciphertext_and_binds_it_to_the_row() {
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .expect("schema");
        harden_schema(&db).expect("harden schema");
        let crypto = Crypto::from_master([6_u8; 32]).expect("crypto");
        let (cipher, nonce) = crypto.encrypt("legacy cifrato").expect("encrypt v1");
        db.execute(
            "INSERT INTO messages (id,session_id,role,content,created_at,content_cipher,content_nonce,encryption_version)
             VALUES ('v1','default','assistant','','now',?1,?2,1)",
            params![cipher, nonce],
        )
        .expect("insert v1");
        assert_eq!(
            migrate_messages_to_v2(&mut db, &crypto).expect("migration"),
            1
        );
        let (cipher, nonce): (Vec<u8>, Vec<u8>) = db
            .query_row(
                "SELECT content_cipher,content_nonce FROM messages WHERE id='v1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("row");
        let aad = message_aad("v1", "assistant", "now").expect("aad");
        assert_eq!(
            crypto
                .decrypt_with_aad(&cipher, &nonce, &aad)
                .expect("decrypt v2"),
            "legacy cifrato"
        );
    }
}
