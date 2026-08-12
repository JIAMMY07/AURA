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
const MAX_TITLE_CHARS: usize = 80;
const MAX_PDF_BYTES: usize = 16 * 1024 * 1024;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    id: String,
    project_id: String,
    title: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyStatus {
    database_integrity: bool,
    secure_delete: bool,
    foreign_keys: bool,
    wal_enabled: bool,
    encrypted_payloads: bool,
    plaintext_payload_rows: u64,
    audit_chain_valid: bool,
    audited_events: usize,
    key_store: &'static str,
    metadata_notice: &'static str,
}

impl AppState {
    pub fn open(app_data: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        fs::create_dir_all(app_data)?;
        let crypto = Crypto::load_or_create().map_err(std::io::Error::other)?;
        let mut database = Connection::open(app_data.join("aura.db"))?;
        configure_database(&database)?;
        database.execute_batch(include_str!("../../migrations/0001_initial.sql"))?;
        harden_schema(&database)?;
        let migrated_metadata = migrate_workspace_metadata(&mut database, &crypto)?;
        let migrated_messages = migrate_messages_to_v2(&mut database, &crypto)?;
        let migrated_audit = super::audit::migrate_legacy_audit(&mut database, &crypto)?;
        database.pragma_update(None, "user_version", 2)?;
        if migrated_metadata > 0 || migrated_messages > 0 || migrated_audit > 0 {
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
pub fn storage_list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectSummary>, String> {
    super::policy::enforce_capability("conversation.read", "allow")?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let mut statement = db
        .prepare(
            "SELECT id, created_at, updated_at, name_cipher, name_nonce
         FROM projects ORDER BY updated_at DESC, created_at ASC",
        )
        .map_err(redact_db)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Vec<u8>>(4)?,
            ))
        })
        .map_err(redact_db)?;
    let mut projects = Vec::new();
    for row in rows {
        let (id, created_at, updated_at, cipher, nonce) = row.map_err(redact_db)?;
        let name = state.crypto.decrypt_with_aad(
            &cipher,
            &nonce,
            &metadata_aad("project", &id, &created_at)?,
        )?;
        projects.push(ProjectSummary {
            id,
            name,
            created_at,
            updated_at,
        });
    }
    Ok(projects)
}

#[tauri::command]
pub fn storage_create_project(
    name: String,
    state: State<'_, AppState>,
) -> Result<ProjectSummary, String> {
    super::policy::enforce_capability("conversation.write", "allow")?;
    let name = validate_title(&name, "Nome progetto")?;
    let id = uuid::Uuid::new_v4().to_string();
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let created_at = database_timestamp(&db)?;
    let (cipher, nonce) = state
        .crypto
        .encrypt_with_aad(&name, &metadata_aad("project", &id, &created_at)?)?;
    db.execute(
        "INSERT INTO projects (id,name,created_at,updated_at,name_cipher,name_nonce,encryption_version)
         VALUES (?1,'',?2,?2,?3,?4,1)", params![id, created_at, cipher, nonce],
    ).map_err(redact_db)?;
    Ok(ProjectSummary {
        id,
        name,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

#[tauri::command]
pub fn storage_list_conversations(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ConversationSummary>, String> {
    super::policy::enforce_capability("conversation.read", "allow")?;
    validate_identifier(&project_id)?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let mut statement = db
        .prepare(
            "SELECT id, project_id, created_at, updated_at, title_cipher, title_nonce
         FROM sessions WHERE project_id=?1 ORDER BY updated_at DESC, created_at ASC",
        )
        .map_err(redact_db)?;
    let rows = statement
        .query_map([&project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, Vec<u8>>(5)?,
            ))
        })
        .map_err(redact_db)?;
    let mut conversations = Vec::new();
    for row in rows {
        let (id, project_id, created_at, updated_at, cipher, nonce) = row.map_err(redact_db)?;
        let title = state.crypto.decrypt_with_aad(
            &cipher,
            &nonce,
            &metadata_aad("conversation", &id, &created_at)?,
        )?;
        conversations.push(ConversationSummary {
            id,
            project_id,
            title,
            created_at,
            updated_at,
        });
    }
    Ok(conversations)
}

#[tauri::command]
pub fn storage_create_conversation(
    project_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, String> {
    super::policy::enforce_capability("conversation.write", "allow")?;
    validate_identifier(&project_id)?;
    let title = validate_title(&title, "Titolo chat")?;
    let id = uuid::Uuid::new_v4().to_string();
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let created_at = database_timestamp(&db)?;
    let (cipher, nonce) = state
        .crypto
        .encrypt_with_aad(&title, &metadata_aad("conversation", &id, &created_at)?)?;
    let project_exists = db
        .query_row("SELECT 1 FROM projects WHERE id=?1", [&project_id], |_| {
            Ok(())
        })
        .optional()
        .map_err(redact_db)?
        .is_some();
    if !project_exists {
        return Err("Progetto non trovato.".to_string());
    }
    db.execute(
        "INSERT INTO sessions (id,project_id,title,created_at,updated_at,title_cipher,title_nonce,encryption_version)
         VALUES (?1,?2,'',?3,?3,?4,?5,1)", params![id, project_id, created_at, cipher, nonce],
    ).map_err(redact_db)?;
    db.execute(
        "UPDATE projects SET updated_at=?1 WHERE id=?2",
        params![created_at, project_id],
    )
    .map_err(redact_db)?;
    Ok(ConversationSummary {
        id,
        project_id,
        title,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

#[tauri::command]
pub fn storage_rename_conversation(
    session_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    super::policy::enforce_capability("conversation.write", "allow")?;
    validate_identifier(&session_id)?;
    let title = validate_title(&title, "Titolo chat")?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let created_at = db
        .query_row(
            "SELECT created_at FROM sessions WHERE id=?1",
            [&session_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(redact_db)?
        .ok_or_else(|| "Conversazione non trovata.".to_string())?;
    let (cipher, nonce) = state.crypto.encrypt_with_aad(
        &title,
        &metadata_aad("conversation", &session_id, &created_at)?,
    )?;
    db.execute("UPDATE sessions SET title='',title_cipher=?1,title_nonce=?2,encryption_version=1,updated_at=datetime('now') WHERE id=?3", params![cipher, nonce, session_id]).map_err(redact_db)?;
    Ok(true)
}

#[tauri::command]
pub fn storage_delete_conversation(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    super::policy::enforce_capability("conversation.clear", "confirm")?;
    validate_identifier(&session_id)?;
    if !super::pc::confirm_native_action(
        "Conferma eliminazione chat",
        "Cancellare definitivamente questa chat e tutti i messaggi cifrati associati? L'azione non puo essere annullata.",
    ) {
        super::audit::append_audit(
            &state,
            "conversation-delete",
            "conversation.delete_cancelled",
            "confirm",
            "Eliminazione chat annullata dall'utente",
            r#"{"contentLogged":false}"#,
        )?;
        return Ok(false);
    }
    let deleted = {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.execute("DELETE FROM sessions WHERE id=?1", [&session_id])
            .map_err(redact_db)?
            > 0
    };
    if deleted {
        super::audit::append_audit(
            &state,
            "conversation-delete",
            "conversation.deleted",
            "confirm",
            "Chat e messaggi cifrati eliminati",
            r#"{"contentLogged":false}"#,
        )?;
    }
    Ok(deleted)
}

#[tauri::command]
pub fn storage_load(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    super::policy::enforce_capability("conversation.read", "allow")?;
    validate_identifier(&session_id)?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let mut statement = db
        .prepare(
            "SELECT id, role, content, created_at, content_cipher, content_nonce, encryption_version
             FROM messages WHERE session_id=?1 ORDER BY sequence DESC LIMIT ?2",
        )
        .map_err(redact_db)?;
    let rows = statement
        .query_map(params![session_id, MAX_MESSAGES as i64], |row| {
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
                &message_aad(&session_id, &id, &role, &created_at)?,
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
pub fn storage_save(
    session_id: String,
    messages: Vec<ChatMessage>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    super::policy::enforce_capability("conversation.write", "allow")?;
    validate_identifier(&session_id)?;
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
        .execute("DELETE FROM messages WHERE session_id=?1", [&session_id])
        .map_err(redact_db)?;
    for message in retained.iter().rev() {
        let aad = message_aad(&session_id, &message.id, &message.role, &message.created_at)?;
        let (ciphertext, nonce) = state.crypto.encrypt_with_aad(&message.content, &aad)?;
        transaction
            .execute(
                "INSERT INTO messages
                 (id, session_id, role, content, created_at, content_cipher, content_nonce, encryption_version)
                 VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, 2)",
                params![message.id, session_id, message.role, message.created_at, ciphertext, nonce],
            )
            .map_err(redact_db)?;
    }
    transaction
        .execute(
            "UPDATE sessions SET updated_at=datetime('now') WHERE id=?1",
            [&session_id],
        )
        .map_err(redact_db)?;
    transaction.commit().map_err(redact_db)
}

#[tauri::command]
pub fn storage_clear(session_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    super::policy::enforce_capability("conversation.clear", "confirm")?;
    validate_identifier(&session_id)?;
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
        db.execute("DELETE FROM messages WHERE session_id=?1", [&session_id])
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

#[tauri::command]
pub fn storage_export_pdf(
    file_name: String,
    bytes: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    super::policy::enforce_capability("conversation.read", "allow")?;
    if bytes.len() < 8 || bytes.len() > MAX_PDF_BYTES || !bytes.starts_with(b"%PDF-") {
        return Err("Documento PDF non valido o troppo grande.".to_string());
    }
    let safe = sanitize_pdf_name(&file_name);
    let documents = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|path| path.join("Documents"))
        .unwrap_or_else(|| state.app_data.clone());
    let export_dir = documents.join("AURA").join("Esportazioni");
    fs::create_dir_all(&export_dir)
        .map_err(|_| "Impossibile creare la cartella di esportazione.".to_string())?;
    let path = unique_export_path(&export_dir, &safe);
    fs::write(&path, bytes).map_err(|_| "Impossibile salvare il PDF.".to_string())?;
    super::audit::append_audit(
        &state,
        "conversation-export",
        "conversation.pdf_exported",
        "allow",
        "Conversazione esportata in PDF",
        r#"{"contentLogged":false}"#,
    )?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn privacy_status(state: State<'_, AppState>) -> Result<PrivacyStatus, String> {
    super::policy::enforce_capability("privacy.inspect", "allow")?;
    let (database_integrity, secure_delete, foreign_keys, wal_enabled, plaintext_payload_rows) = {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        let integrity = db
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .map_err(redact_db)?
            .eq_ignore_ascii_case("ok");
        let secure_delete = db
            .pragma_query_value(None, "secure_delete", |row| row.get::<_, i64>(0))
            .map_err(redact_db)?
            == 1;
        let foreign_keys = db
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))
            .map_err(redact_db)?
            == 1;
        let wal_enabled = db
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
            .map_err(redact_db)?
            .eq_ignore_ascii_case("wal");
        let plaintext_payload_rows = count_unencrypted_payloads(&db)?;
        (
            integrity,
            secure_delete,
            foreign_keys,
            wal_enabled,
            plaintext_payload_rows,
        )
    };
    let audit = super::audit::verify_audit(&state)?;
    append_privacy_audit(
        &state,
        database_integrity,
        plaintext_payload_rows,
        audit.valid,
    )?;
    Ok(PrivacyStatus {
        database_integrity,
        secure_delete,
        foreign_keys,
        wal_enabled,
        encrypted_payloads: plaintext_payload_rows == 0,
        plaintext_payload_rows,
        audit_chain_valid: audit.valid,
        audited_events: audit.count,
        key_store: "Windows Credential Manager",
        metadata_notice: "Solo identificatori tecnici, date, nomi sorgente e URL dei connettori restano metadati locali non cifrati; messaggi, titoli di chat, nomi progetto e segreti sono cifrati.",
    })
}

fn count_unencrypted_payloads(db: &Connection) -> Result<u64, String> {
    let queries = [
        "SELECT COUNT(*) FROM messages WHERE content<>'' OR encryption_version<>2 OR content_cipher IS NULL OR content_nonce IS NULL",
        "SELECT COUNT(*) FROM projects WHERE name<>'' OR encryption_version<>1 OR name_cipher IS NULL OR name_nonce IS NULL",
        "SELECT COUNT(*) FROM sessions WHERE title<>'' OR encryption_version<>1 OR title_cipher IS NULL OR title_nonce IS NULL",
        "SELECT COUNT(*) FROM agent_profile WHERE encryption_version<>1 OR length(profile_cipher)=0 OR length(profile_nonce)<>12",
        "SELECT COUNT(*) FROM agent_feedback WHERE encryption_version<>1 OR length(correction_cipher)=0 OR length(correction_nonce)<>12",
        "SELECT COUNT(*) FROM agent_memory_snapshots WHERE encryption_version<>1 OR length(memory_cipher)=0 OR length(memory_nonce)<>12",
        "SELECT COUNT(*) FROM api_connectors WHERE length(secret_cipher)=0 OR length(secret_nonce)<>12",
        "SELECT COUNT(*) FROM rag_documents WHERE length(source_cipher)=0 OR length(source_nonce)<>12",
        "SELECT COUNT(*) FROM rag_chunks WHERE length(content_cipher)=0 OR length(content_nonce)<>12 OR length(embedding_cipher)=0 OR length(embedding_nonce)<>12",
        "SELECT COUNT(*) FROM remote_pairing WHERE length(secret_cipher)=0 OR length(secret_nonce)<>12",
    ];
    queries.iter().try_fold(0_u64, |total, query| {
        db.query_row(query, [], |row| row.get::<_, u64>(0))
            .map(|count| total + count)
            .map_err(redact_db)
    })
}

fn append_privacy_audit(
    state: &AppState,
    database_integrity: bool,
    plaintext_payload_rows: u64,
    audit_chain_valid: bool,
) -> Result<(), String> {
    super::audit::append_audit(
        state,
        "privacy-self-check",
        "privacy.verified",
        if database_integrity && plaintext_payload_rows == 0 && audit_chain_valid {
            "allow"
        } else {
            "caution"
        },
        "Autodiagnosi privacy locale completata",
        &serde_json::json!({
            "databaseIntegrity": database_integrity,
            "plaintextPayloadRows": plaintext_payload_rows,
            "auditChainValidBeforeCheck": audit_chain_valid,
            "contentLogged": false
        })
        .to_string(),
    )
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

fn validate_identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err("Identificatore conversazione non valido.".to_string());
    }
    Ok(())
}

fn validate_title(value: &str, label: &str) -> Result<String, String> {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty()
        || compact.chars().count() > MAX_TITLE_CHARS
        || compact.chars().any(char::is_control)
    {
        return Err(format!(
            "{label} non valido: usa da 1 a {MAX_TITLE_CHARS} caratteri."
        ));
    }
    Ok(compact)
}

fn database_timestamp(db: &Connection) -> Result<String, String> {
    db.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
        row.get(0)
    })
    .map_err(redact_db)
}

fn metadata_aad(kind: &str, id: &str, created_at: &str) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&serde_json::json!({
        "version": 1, "kind": kind, "id": id, "createdAt": created_at
    }))
    .map_err(|_| "Metadati workspace non serializzabili.".to_string())
}

fn sanitize_pdf_name(value: &str) -> String {
    let trimmed = value.trim();
    let stem = if trimmed.to_ascii_lowercase().ends_with(".pdf") {
        &trimmed[..trimmed.len() - 4]
    } else {
        trimmed
    };
    let safe = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{}.pdf",
        if safe.is_empty() {
            "Conversazione AURA"
        } else {
            &safe
        }
    )
}

fn unique_export_path(directory: &Path, name: &str) -> PathBuf {
    let initial = directory.join(name);
    if !initial.exists() {
        return initial;
    }
    let stem = name.strip_suffix(".pdf").unwrap_or(name);
    for suffix in 2..10_000 {
        let candidate = directory.join(format!("{stem} ({suffix}).pdf"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem}-{}.pdf", uuid::Uuid::new_v4()))
}

pub(crate) fn harden_schema(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            name_cipher BLOB,
            name_nonce BLOB,
            encryption_version INTEGER NOT NULL DEFAULT 0
         );
         INSERT OR IGNORE INTO projects (id,name,created_at,updated_at)
         VALUES ('default','Progetto principale',datetime('now'),datetime('now'));",
    )?;
    add_column_if_missing(
        db,
        "sessions",
        "project_id",
        "TEXT NOT NULL DEFAULT 'default'",
    )?;
    add_column_if_missing(db, "sessions", "title_cipher", "BLOB")?;
    add_column_if_missing(db, "sessions", "title_nonce", "BLOB")?;
    add_column_if_missing(
        db,
        "sessions",
        "encryption_version",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    db.execute_batch(
        "UPDATE sessions SET project_id='default' WHERE project_id IS NULL OR project_id='';
         CREATE INDEX IF NOT EXISTS idx_sessions_project_updated ON sessions(project_id, updated_at DESC);",
    )?;
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
         CREATE INDEX IF NOT EXISTS idx_agent_feedback_created ON agent_feedback(created_at DESC);
         CREATE TABLE IF NOT EXISTS agent_memory_snapshots (
            id TEXT PRIMARY KEY,
            memory_cipher BLOB NOT NULL,
            memory_nonce BLOB NOT NULL,
            encryption_version INTEGER NOT NULL,
            lesson_count INTEGER NOT NULL,
            created_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_agent_memory_created ON agent_memory_snapshots(created_at DESC);",
    )?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_memory_control (
            id TEXT PRIMARY KEY CHECK(id='default'),
            automatic INTEGER NOT NULL DEFAULT 1 CHECK(automatic IN (0,1))
         );
         INSERT OR IGNORE INTO agent_memory_control (id,automatic) VALUES ('default',1);",
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
    add_column_if_missing(db, "rag_documents", "content_sha256", "TEXT")?;
    db.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_rag_documents_content_sha256
         ON rag_documents(content_sha256) WHERE content_sha256 IS NOT NULL;",
    )?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS remote_pairing (
            id TEXT PRIMARY KEY CHECK(id='default'),
            device_name TEXT NOT NULL,
            relay_url TEXT NOT NULL,
            secret_cipher BLOB NOT NULL,
            secret_nonce BLOB NOT NULL,
            enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
            created_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS remote_nonces (
            nonce TEXT PRIMARY KEY,
            command_id TEXT NOT NULL,
            seen_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_remote_nonces_seen ON remote_nonces(seen_at);",
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
            "SELECT id, session_id, role, content, created_at, content_cipher, content_nonce, encryption_version
             FROM messages WHERE encryption_version<>2 OR content_cipher IS NULL",
        )?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<Vec<u8>>>(5)?,
                    row.get::<_, Option<Vec<u8>>>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let count = rows.len();
    let transaction = db.transaction()?;
    for (id, session_id, role, legacy_content, created_at, old_cipher, old_nonce, version) in rows {
        let content = match (version, old_cipher, old_nonce) {
            (1, Some(cipher), Some(nonce)) => crypto
                .decrypt(&cipher, &nonce)
                .map_err(std::io::Error::other)?,
            (0, _, _) => legacy_content,
            _ => return Err("Stato cifratura legacy non valido.".into()),
        };
        let aad =
            message_aad(&session_id, &id, &role, &created_at).map_err(std::io::Error::other)?;
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

fn message_aad(
    session_id: &str,
    id: &str,
    role: &str,
    created_at: &str,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&serde_json::json!({
        "version": 2,
        "sessionId": session_id,
        "messageId": id,
        "role": role,
        "createdAt": created_at,
    }))
    .map_err(|_| "Metadati messaggio non serializzabili.".to_string())
}

fn migrate_workspace_metadata(
    db: &mut Connection,
    crypto: &Crypto,
) -> Result<usize, Box<dyn std::error::Error>> {
    let projects = {
        let mut statement = db.prepare("SELECT id,name,created_at FROM projects WHERE encryption_version<>1 OR name_cipher IS NULL")?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let sessions = {
        let mut statement = db.prepare("SELECT id,title,created_at FROM sessions WHERE encryption_version<>1 OR title_cipher IS NULL")?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let count = projects.len() + sessions.len();
    let transaction = db.transaction()?;
    for (id, name, created_at) in projects {
        let name = if name.trim().is_empty() {
            "Progetto principale"
        } else {
            name.trim()
        };
        let (cipher, nonce) = crypto
            .encrypt_with_aad(
                name,
                &metadata_aad("project", &id, &created_at).map_err(std::io::Error::other)?,
            )
            .map_err(std::io::Error::other)?;
        transaction.execute("UPDATE projects SET name='',name_cipher=?1,name_nonce=?2,encryption_version=1 WHERE id=?3", params![cipher, nonce, id])?;
    }
    for (id, title, created_at) in sessions {
        let title = if title.trim().is_empty() {
            "Conversazione principale"
        } else {
            title.trim()
        };
        let (cipher, nonce) = crypto
            .encrypt_with_aad(
                title,
                &metadata_aad("conversation", &id, &created_at).map_err(std::io::Error::other)?,
            )
            .map_err(std::io::Error::other)?;
        transaction.execute("UPDATE sessions SET title='',title_cipher=?1,title_nonce=?2,encryption_version=1 WHERE id=?3", params![cipher, nonce, id])?;
    }
    transaction.commit()?;
    Ok(count)
}

pub(crate) fn redact_db(_: rusqlite::Error) -> String {
    "Operazione database locale fallita.".to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        configure_database, count_unencrypted_payloads, harden_schema, message_aad,
        migrate_messages_to_v2, migrate_workspace_metadata,
    };
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
    fn privacy_self_check_detects_legacy_plaintext_rows() {
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .expect("schema");
        harden_schema(&db).expect("harden schema");
        let crypto = Crypto::from_master([3_u8; 32]).expect("crypto");
        migrate_workspace_metadata(&mut db, &crypto).expect("metadata migration");
        assert_eq!(count_unencrypted_payloads(&db).expect("empty check"), 0);
        db.execute(
            "INSERT INTO messages (id,session_id,role,content,created_at) VALUES ('leak','default','user','segreto','now')",
            [],
        )
        .expect("legacy row");
        assert_eq!(count_unencrypted_payloads(&db).expect("leak check"), 1);
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
        let aad = message_aad("default", "1", "user", "now").expect("aad");
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
        let aad = message_aad("default", "v1", "assistant", "now").expect("aad");
        assert_eq!(
            crypto
                .decrypt_with_aad(&cipher, &nonce, &aad)
                .expect("decrypt v2"),
            "legacy cifrato"
        );
    }
}
