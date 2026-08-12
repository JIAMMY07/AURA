use super::{
    audit::append_audit,
    pc::confirm_native_action,
    policy::enforce_capability,
    storage::{redact_db, AppState},
};
use quick_xml::{events::Event, Reader};
use reqwest::redirect::Policy;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, collections::HashSet, fs, io::Read, path::Path, time::Duration};
use tauri::State;
use uuid::Uuid;

const EMBEDDING_MODEL: &str = "qwen3-embedding:0.6b";
const MAX_TEXT_DOCUMENT_BYTES: u64 = 512 * 1024;
const MAX_BINARY_DOCUMENT_BYTES: u64 = 10 * 1024 * 1024;
const MAX_EXTRACTED_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CHUNKS: usize = 256;
const EMBEDDING_BATCH: usize = 32;
const MAX_SEARCH_CHUNKS: usize = 10_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RagDocumentView {
    id: String,
    source_name: String,
    chunk_count: usize,
    created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RagImportResult {
    id: String,
    source_name: String,
    chunk_count: usize,
    model: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RagHit {
    document_id: String,
    source_name: String,
    ordinal: usize,
    score: f32,
    excerpt: String,
    citation: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[tauri::command]
pub fn rag_list_documents(state: State<'_, AppState>) -> Result<Vec<RagDocumentView>, String> {
    enforce_capability("rag.read", "allow")?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let mut s=db.prepare("SELECT id,source_name,chunk_count,created_at FROM rag_documents ORDER BY created_at DESC").map_err(redact_db)?;
    let rows = s
        .query_map([], |r| {
            Ok(RagDocumentView {
                id: r.get(0)?,
                source_name: r.get(1)?,
                chunk_count: r.get::<_, i64>(2)? as usize,
                created_at: r.get(3)?,
            })
        })
        .map_err(redact_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(redact_db)?;
    Ok(rows)
}

#[tauri::command]
pub async fn rag_import_document(
    path: String,
    state: State<'_, AppState>,
) -> Result<RagImportResult, String> {
    enforce_capability("rag.import", "confirm")?;
    let source = Path::new(&path);
    let metadata = fs::metadata(source).map_err(|_| "Documento non trovato.".to_string())?;
    if !metadata.is_file() {
        return Err("Documento assente o non leggibile.".to_string());
    }
    let extension = source
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        extension.as_str(),
        "txt"
            | "md"
            | "json"
            | "csv"
            | "tsv"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "py"
            | "toml"
            | "yaml"
            | "yml"
            | "pdf"
            | "docx"
    ) {
        return Err(
            "Formato RAG non supportato: usa PDF, DOCX, testo, Markdown, dati o codice sorgente."
                .to_string(),
        );
    }
    let binary = matches!(extension.as_str(), "pdf" | "docx");
    let size_limit = if binary {
        MAX_BINARY_DOCUMENT_BYTES
    } else {
        MAX_TEXT_DOCUMENT_BYTES
    };
    if metadata.len() > size_limit {
        return Err(format!(
            "Documento oltre il limite di {} MiB.",
            size_limit / 1024 / 1024
        ));
    }
    let name = source
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("documento")
        .chars()
        .take(200)
        .collect::<String>();
    if !confirm_native_action(
        "Conferma importazione RAG",
        &format!(
            "Importare e indicizzare localmente '{name}'? Contenuto ed embeddings saranno cifrati."
        ),
    ) {
        return Err("Importazione RAG annullata.".to_string());
    }
    let content = extract_document_text(source, &extension)?;
    let content_sha256 = hex::encode(Sha256::digest(content.as_bytes()));
    {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        let duplicate = db
            .query_row(
                "SELECT source_name FROM rag_documents WHERE content_sha256=?1",
                [&content_sha256],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(redact_db)?;
        if let Some(existing) = duplicate {
            return Err(format!("Documento già indicizzato come '{existing}'."));
        }
    }
    let chunks = chunk_text(&content)?;
    if chunks.is_empty() {
        return Err("Documento senza testo indicizzabile.".to_string());
    }
    let embeddings = embed(chunks.clone()).await?;
    if embeddings.len() != chunks.len() {
        return Err("Numero embeddings Ollama incoerente.".to_string());
    }
    let id = Uuid::new_v4().to_string();
    let source_aad = format!("rag-source-v1|{id}");
    let (source_cipher, source_nonce) = state
        .crypto
        .encrypt_with_aad(&path, source_aad.as_bytes())?;
    let mut db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let tx = db.transaction().map_err(redact_db)?;
    tx.execute("INSERT INTO rag_documents(id,source_name,source_cipher,source_nonce,chunk_count,created_at,content_sha256) VALUES(?1,?2,?3,?4,?5,datetime('now'),?6)",params![id,name,source_cipher,source_nonce,chunks.len() as i64,content_sha256]).map_err(redact_db)?;
    for (ordinal, (text, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
        let chunk_id = Uuid::new_v4().to_string();
        let aad = format!("rag-chunk-v1|{id}|{ordinal}");
        let (cc, cn) = state.crypto.encrypt_with_aad(text, aad.as_bytes())?;
        let encoded = serde_json::to_string(embedding)
            .map_err(|_| "Embedding non serializzabile.".to_string())?;
        let (ec, en) = state.crypto.encrypt_with_aad(&encoded, aad.as_bytes())?;
        tx.execute("INSERT INTO rag_chunks(id,document_id,ordinal,content_cipher,content_nonce,embedding_cipher,embedding_nonce) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![chunk_id,id,ordinal as i64,cc,cn,ec,en]).map_err(redact_db)?;
    }
    tx.commit().map_err(redact_db)?;
    append_audit(
        &state,
        "rag-import",
        "rag.imported",
        "confirm",
        "Documento RAG cifrato indicizzato",
        &serde_json::json!({"id":id,"sourceName":name,"chunks":chunks.len(),"pathLogged":false})
            .to_string(),
    )?;
    Ok(RagImportResult {
        id,
        source_name: name,
        chunk_count: chunks.len(),
        model: EMBEDDING_MODEL,
    })
}

fn extract_document_text(path: &Path, extension: &str) -> Result<String, String> {
    let content = match extension {
        "pdf" => pdf_extract::extract_text(path)
            .map_err(|_| "PDF non leggibile o senza testo estraibile.".to_string())?,
        "docx" => extract_docx(path)?,
        _ => fs::read_to_string(path)
            .map_err(|_| "Documento testuale non UTF-8 leggibile.".to_string())?,
    };
    if content.len() > MAX_EXTRACTED_TEXT_BYTES {
        return Err("Testo estratto oltre il limite di 2 MiB.".to_string());
    }
    Ok(content)
}

fn extract_docx(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|_| "DOCX non leggibile.".to_string())?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| "Archivio DOCX non valido.".to_string())?;
    let document = archive
        .by_name("word/document.xml")
        .map_err(|_| "DOCX privo del documento principale.".to_string())?;
    if document.size() > MAX_EXTRACTED_TEXT_BYTES as u64 {
        return Err("XML DOCX oltre il limite di 2 MiB.".to_string());
    }
    let mut xml = Vec::new();
    document
        .take(MAX_EXTRACTED_TEXT_BYTES as u64 + 1)
        .read_to_end(&mut xml)
        .map_err(|_| "Lettura DOCX fallita.".to_string())?;
    if xml.len() > MAX_EXTRACTED_TEXT_BYTES {
        return Err("DOCX espanso oltre il limite consentito.".to_string());
    }
    let mut reader = Reader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(false);
    let mut output = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(text)) => {
                let decoded = text
                    .decode()
                    .map_err(|_| "Testo DOCX non valido.".to_string())?;
                output.push_str(&decoded);
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Some(character) = reference
                    .resolve_char_ref()
                    .map_err(|_| "Riferimento carattere DOCX non valido.".to_string())?
                {
                    output.push(character);
                } else {
                    let name = reference
                        .decode()
                        .map_err(|_| "Entità DOCX non valida.".to_string())?;
                    output.push(match name.as_ref() {
                        "amp" => '&',
                        "lt" => '<',
                        "gt" => '>',
                        "quot" => '"',
                        "apos" => '\'',
                        _ => return Err("Entità DOCX non supportata.".to_string()),
                    });
                }
            }
            Ok(Event::End(end)) if end.name().as_ref() == b"w:p" => output.push('\n'),
            Ok(Event::Empty(empty))
                if matches!(empty.name().as_ref(), b"w:tab" | b"w:br" | b"w:cr") =>
            {
                output.push(' ')
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err("XML DOCX malformato.".to_string()),
            _ => {}
        }
    }
    Ok(output)
}

#[tauri::command]
pub async fn rag_search(
    query: String,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<RagHit>, String> {
    enforce_capability("rag.read", "allow")?;
    if query.trim().len() < 2 || query.len() > 4000 || !(1..=10).contains(&limit) {
        return Err("Ricerca RAG fuori dai limiti.".to_string());
    }
    let query_terms = lexical_terms(&query);
    let query_embedding = embed(vec![query.clone()])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| "Embedding query assente.".to_string())?;
    let rows = {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        let total = db
            .query_row("SELECT COUNT(*) FROM rag_chunks", [], |row| {
                row.get::<_, usize>(0)
            })
            .map_err(redact_db)?;
        if total > MAX_SEARCH_CHUNKS {
            return Err(format!("Indice RAG oltre il limite di ricerca locale ({MAX_SEARCH_CHUNKS} chunk): suddividi l'archivio."));
        }
        let mut s=db.prepare("SELECT c.document_id,d.source_name,c.ordinal,c.content_cipher,c.content_nonce,c.embedding_cipher,c.embedding_nonce FROM rag_chunks c JOIN rag_documents d ON d.id=c.document_id").map_err(redact_db)?;
        let rows = s
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                    r.get::<_, Vec<u8>>(4)?,
                    r.get::<_, Vec<u8>>(5)?,
                    r.get::<_, Vec<u8>>(6)?,
                ))
            })
            .map_err(redact_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(redact_db)?;
        rows
    };
    let mut hits = Vec::new();
    for (document_id, source_name, ordinal, cc, cn, ec, en) in rows {
        let aad = format!("rag-chunk-v1|{document_id}|{ordinal}");
        let text = state.crypto.decrypt_with_aad(&cc, &cn, aad.as_bytes())?;
        let encoded = state.crypto.decrypt_with_aad(&ec, &en, aad.as_bytes())?;
        let vector: Vec<f32> = serde_json::from_str(&encoded)
            .map_err(|_| "Embedding cifrato non valido.".to_string())?;
        let semantic = cosine(&query_embedding, &vector);
        let lexical = lexical_overlap(&query_terms, &text);
        let score = if semantic < 0.0 {
            semantic
        } else {
            semantic * 0.9 + lexical * 0.1
        };
        hits.push(RagHit {
            document_id,
            source_name: source_name.clone(),
            ordinal: ordinal as usize,
            score,
            excerpt: text.chars().take(1200).collect(),
            citation: format!("[{} — frammento {}]", source_name, ordinal + 1),
        });
    }
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
    hits.truncate(limit);
    append_audit(
        &state,
        "rag-search",
        "rag.searched",
        "allow",
        "Ricerca semantica locale completata",
        &serde_json::json!({"results":hits.len(),"queryLogged":false}).to_string(),
    )?;
    Ok(hits)
}

#[tauri::command]
pub fn rag_delete_document(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("rag.delete", "confirm")?;
    if Uuid::parse_str(&id).is_err() {
        return Err("ID documento non valido.".to_string());
    }
    let name = {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.query_row(
            "SELECT source_name FROM rag_documents WHERE id=?1",
            [&id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(redact_db)?
        .ok_or_else(|| "Documento non trovato.".to_string())?
    };
    if !confirm_native_action(
        "Conferma eliminazione RAG",
        &format!("Eliminare indice cifrato di '{name}'?"),
    ) {
        return Ok(false);
    }
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let changed = db
        .execute("DELETE FROM rag_documents WHERE id=?1", [&id])
        .map_err(redact_db)?
        > 0;
    append_audit(
        &state,
        "rag-delete",
        "rag.deleted",
        "confirm",
        "Documento RAG eliminato",
        &serde_json::json!({"id":id}).to_string(),
    )?;
    Ok(changed)
}

async fn embed(input: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(120))
        .redirect(Policy::none())
        .build()
        .map_err(|_| "Client embeddings non disponibile.".to_string())?;
    let mut all = Vec::with_capacity(input.len());
    let batches = input.chunks(EMBEDDING_BATCH).collect::<Vec<_>>();
    for (index, batch) in batches.iter().enumerate() {
        let keep_alive = if index + 1 == batches.len() {
            "0"
        } else {
            "5m"
        };
        let response=client.post("http://127.0.0.1:11434/api/embed").json(&serde_json::json!({"model":EMBEDDING_MODEL,"input":batch,"keep_alive":keep_alive,"options":{"num_ctx":2048,"num_gpu":0}})).send().await.map_err(|_|"Ollama embeddings non raggiungibile.".to_string())?;
        if !response.status().is_success() {
            return Err("Modello embeddings locale non disponibile.".to_string());
        }
        let mut embeddings = response
            .json::<EmbedResponse>()
            .await
            .map_err(|_| "Risposta embeddings non valida.".to_string())?
            .embeddings;
        if embeddings.len() != batch.len() {
            return Err("Numero embeddings Ollama incoerente.".to_string());
        }
        all.append(&mut embeddings);
    }
    Ok(all)
}
fn chunk_text(input: &str) -> Result<Vec<String>, String> {
    let chars = input.chars().collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        if out.len() >= MAX_CHUNKS {
            return Err(format!("Documento richiede più di {MAX_CHUNKS} chunk: importazione annullata senza troncare contenuto."));
        }
        let end = (start + 1600).min(chars.len());
        let chunk = chars[start..end]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        if !chunk.is_empty() {
            out.push(chunk);
        }
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(200);
    }
    Ok(out)
}

fn lexical_terms(input: &str) -> HashSet<String> {
    input
        .split(|character: char| !character.is_alphanumeric())
        .map(|term| term.to_lowercase())
        .filter(|term| term.chars().count() >= 3)
        .take(128)
        .collect()
}

fn lexical_overlap(query: &HashSet<String>, text: &str) -> f32 {
    if query.is_empty() {
        return 0.0;
    }
    let text = lexical_terms(text);
    query.iter().filter(|term| text.contains(*term)).count() as f32 / query.len() as f32
}
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return -1.0;
    }
    let (dot, aa, bb) = a.iter().zip(b).fold((0.0, 0.0, 0.0), |(d, x, y), (i, j)| {
        (d + i * j, x + i * i, y + j * j)
    });
    if aa == 0.0 || bb == 0.0 {
        -1.0
    } else {
        dot / (aa.sqrt() * bb.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::{chunk_text, cosine, extract_docx, lexical_overlap, lexical_terms};
    use std::io::Write;
    #[test]
    fn chunks_with_bounds_and_overlap() {
        let c = chunk_text(&"x".repeat(20_000)).unwrap();
        assert!(c.len() > 1);
        assert!(c.iter().all(|v| v.len() <= 1600));
        assert!(chunk_text(&"x".repeat(500_000)).is_err());
    }
    #[test]
    fn cosine_orders_vectors() {
        assert!(cosine(&[1.0, 0.0], &[1.0, 0.0]) > cosine(&[1.0, 0.0], &[0.0, 1.0]));
    }
    #[test]
    fn lexical_signal_rewards_exact_domain_terms() {
        let query = lexical_terms("codice errore AURA-742");
        assert!(
            lexical_overlap(&query, "Manuale: risolvere errore AURA-742")
                > lexical_overlap(&query, "Testo generico")
        );
    }
    #[test]
    fn extracts_bounded_docx_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.docx");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(br#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>Ciao &amp; AURA</w:t></w:r></w:p></w:body></w:document>"#).unwrap();
        zip.finish().unwrap();
        let text = extract_docx(&path).unwrap();
        assert!(text.contains("Ciao & AURA"), "extracted: {text:?}");
    }
}
