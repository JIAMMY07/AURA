use super::storage::AppState;
use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{ipc::Channel, State};

const OLLAMA_BASE: &str = "http://127.0.0.1:11434";
const MAX_MESSAGES: usize = 100;
const MAX_MESSAGE_BYTES: usize = 128 * 1024;
const MAX_REQUEST_BYTES: usize = 512 * 1024;
const MAX_STREAM_BUFFER: usize = 1024 * 1024;
const SYSTEM_PROMPT: &str = "Sei AURA, un assistente Windows local-first. Rispondi nella lingua dell'utente in modo chiaro e conciso. Non affermare di aver eseguito azioni sul PC se non hai ricevuto un risultato verificato da uno strumento. Non mostrare ragionamenti interni.";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OllamaModel {
    name: String,
    model: String,
    size: u64,
    modified_at: String,
    details: Option<Value>,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Clone, Deserialize, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    request_id: String,
    model: String,
    messages: Vec<ChatMessage>,
    profile: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum OllamaEvent {
    Token { token: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResult {
    tokens_per_second: Option<f64>,
    done_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatChunk {
    message: Option<ChatChunkMessage>,
    done_reason: Option<String>,
    eval_count: Option<u64>,
    eval_duration: Option<u64>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ChatChunkMessage {
    content: Option<String>,
}

#[tauri::command]
pub async fn ollama_health() -> bool {
    let Ok(client) = client(Duration::from_secs(4)) else {
        return false;
    };
    client
        .get(format!("{OLLAMA_BASE}/api/tags"))
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn ollama_list_models() -> Result<Vec<OllamaModel>, String> {
    let models = fetch_models().await?;
    Ok(models.into_iter().filter(is_strictly_local_model).collect())
}

#[tauri::command]
pub async fn ollama_chat(
    request: ChatRequest,
    on_event: Channel<OllamaEvent>,
    state: State<'_, AppState>,
) -> Result<ChatResult, String> {
    validate_request(&request)?;
    let available = fetch_models().await?;
    if !available
        .iter()
        .any(|model| model.name == request.model && is_strictly_local_model(model))
    {
        return Err("Modello non locale o non installato: richiesta bloccata.".to_string());
    }

    let cancellation = Arc::new(AtomicBool::new(false));
    {
        let mut active = state
            .cancellations
            .lock()
            .map_err(|_| "Registro annullamenti non disponibile.".to_string())?;
        if active.contains_key(&request.request_id) {
            return Err("Identificatore generazione già attivo.".to_string());
        }
        active.insert(request.request_id.clone(), Arc::clone(&cancellation));
    }

    let result = stream_chat(&request, on_event, cancellation).await;
    if let Ok(mut active) = state.cancellations.lock() {
        active.remove(&request.request_id);
    }
    result
}

#[tauri::command]
pub fn ollama_cancel(request_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if request_id.is_empty() || request_id.len() > 128 {
        return Err("Identificatore generazione non valido.".to_string());
    }
    let active = state
        .cancellations
        .lock()
        .map_err(|_| "Registro annullamenti non disponibile.".to_string())?;
    if let Some(flag) = active.get(&request_id) {
        flag.store(true, Ordering::Release);
    }
    Ok(())
}

async fn fetch_models() -> Result<Vec<OllamaModel>, String> {
    let response = client(Duration::from_secs(5))?
        .get(format!("{OLLAMA_BASE}/api/tags"))
        .send()
        .await
        .map_err(|_| "Ollama locale non raggiungibile.".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Ollama locale non disponibile ({}).",
            response.status()
        ));
    }
    response
        .json::<TagsResponse>()
        .await
        .map(|payload| payload.models)
        .map_err(|_| "Catalogo modelli Ollama non valido.".to_string())
}

fn is_strictly_local_model(model: &OllamaModel) -> bool {
    let lower = model.name.to_ascii_lowercase();
    let format_is_local = model
        .details
        .as_ref()
        .and_then(|details| details.get("format"))
        .and_then(Value::as_str)
        .is_some_and(|format| format.eq_ignore_ascii_case("gguf"));
    model.size >= 1024 * 1024
        && format_is_local
        && !lower.ends_with("-cloud")
        && !lower.ends_with(":cloud")
        && !lower.contains("-cloud:")
}

async fn stream_chat(
    request: &ChatRequest,
    on_event: Channel<OllamaEvent>,
    cancellation: Arc<AtomicBool>,
) -> Result<ChatResult, String> {
    let (context_tokens, max_output_tokens, temperature, keep_alive) = profile(&request.profile)?;
    let messages = trim_messages(&request.messages, context_tokens, max_output_tokens);
    let mut outbound = Vec::with_capacity(messages.len() + 1);
    outbound.push(serde_json::json!({ "role": "system", "content": SYSTEM_PROMPT }));
    outbound.extend(
        messages
            .iter()
            .map(|message| serde_json::json!({ "role": message.role, "content": message.content })),
    );

    let response = client(Duration::from_secs(180))?
        .post(format!("{OLLAMA_BASE}/api/chat"))
        .json(&serde_json::json!({
            "model": request.model,
            "messages": outbound,
            "stream": true,
            "think": false,
            "keep_alive": keep_alive,
            "options": {
                "num_ctx": context_tokens,
                "num_predict": max_output_tokens,
                "temperature": temperature,
            }
        }))
        .send()
        .await
        .map_err(|_| "Richiesta a Ollama locale fallita.".to_string())?;
    if !response.status().is_success() {
        return Err(format!("Richiesta Ollama fallita ({}).", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = Vec::<u8>::new();
    let mut final_chunk = None;
    while let Some(chunk) = stream.next().await {
        if cancellation.load(Ordering::Acquire) {
            return Err("Generazione annullata.".to_string());
        }
        let chunk = chunk.map_err(|_| "Stream Ollama interrotto.".to_string())?;
        buffer.extend_from_slice(&chunk);
        if buffer.len() > MAX_STREAM_BUFFER {
            return Err("Risposta Ollama oltre il limite consentito.".to_string());
        }
        while let Some(position) = buffer.iter().position(|byte| *byte == b'\n') {
            let line = buffer.drain(..=position).collect::<Vec<_>>();
            if line.iter().all(|byte| byte.is_ascii_whitespace()) {
                continue;
            }
            final_chunk = Some(consume_chunk(&line, &on_event)?);
        }
    }
    if !buffer.iter().all(|byte| byte.is_ascii_whitespace()) {
        final_chunk = Some(consume_chunk(&buffer, &on_event)?);
    }
    let duration = final_chunk
        .as_ref()
        .and_then(|chunk| chunk.eval_duration)
        .map(|nanoseconds| nanoseconds as f64 / 1_000_000_000.0)
        .unwrap_or_default();
    Ok(ChatResult {
        tokens_per_second: final_chunk
            .as_ref()
            .and_then(|chunk| chunk.eval_count)
            .filter(|_| duration > 0.0)
            .map(|count| count as f64 / duration),
        done_reason: final_chunk.and_then(|chunk| chunk.done_reason),
    })
}

fn consume_chunk(line: &[u8], on_event: &Channel<OllamaEvent>) -> Result<ChatChunk, String> {
    let chunk: ChatChunk =
        serde_json::from_slice(line).map_err(|_| "Stream Ollama non valido.".to_string())?;
    if let Some(error) = &chunk.error {
        return Err(format!("Ollama: {error}"));
    }
    if let Some(token) = chunk
        .message
        .as_ref()
        .and_then(|message| message.content.as_ref())
    {
        if !token.is_empty() {
            on_event
                .send(OllamaEvent::Token {
                    token: token.clone(),
                })
                .map_err(|_| "Canale streaming non disponibile.".to_string())?;
        }
    }
    Ok(chunk)
}

fn validate_request(request: &ChatRequest) -> Result<(), String> {
    if request.request_id.is_empty() || request.request_id.len() > 128 {
        return Err("Identificatore generazione non valido.".to_string());
    }
    if request.model.is_empty()
        || request.model.len() > 256
        || request.messages.len() > MAX_MESSAGES
    {
        return Err("Richiesta modello fuori dai limiti.".to_string());
    }
    let mut total = 0_usize;
    for message in &request.messages {
        if !matches!(message.role.as_str(), "user" | "assistant" | "system")
            || message.content.len() > MAX_MESSAGE_BYTES
        {
            return Err("Messaggio Ollama non valido.".to_string());
        }
        total = total.saturating_add(message.content.len());
    }
    if total > MAX_REQUEST_BYTES {
        return Err("Contesto conversazione troppo grande.".to_string());
    }
    profile(&request.profile).map(|_| ())
}

fn profile(name: &str) -> Result<(usize, usize, f32, &'static str), String> {
    match name {
        "Leggero" => Ok((4096, 768, 0.3, "2m")),
        "Bilanciato" => Ok((8192, 1536, 0.5, "5m")),
        "Qualità" => Ok((12288, 2048, 0.6, "10m")),
        _ => Err("Profilo risorse non riconosciuto.".to_string()),
    }
}

fn trim_messages(
    messages: &[ChatMessage],
    context_tokens: usize,
    output_tokens: usize,
) -> Vec<ChatMessage> {
    let character_budget = (context_tokens.saturating_sub(output_tokens + 768) * 3).max(1024);
    let mut selected = Vec::new();
    let mut used = 0_usize;
    for message in messages.iter().rev() {
        let cost = message.content.chars().count() + 32;
        if !selected.is_empty() && used.saturating_add(cost) > character_budget {
            break;
        }
        if selected.is_empty() && cost > character_budget {
            let content = message
                .content
                .chars()
                .rev()
                .take(character_budget)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            selected.push(ChatMessage {
                role: message.role.clone(),
                content,
            });
            break;
        }
        selected.push(message.clone());
        used += cost;
    }
    selected.reverse();
    selected
}

fn client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(timeout)
        .redirect(Policy::none())
        .build()
        .map_err(|_| "Client Ollama locale non disponibile.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{is_strictly_local_model, trim_messages, ChatMessage, OllamaModel};
    use serde_json::json;

    fn model(name: &str, size: u64, format: &str) -> OllamaModel {
        OllamaModel {
            name: name.to_string(),
            model: name.to_string(),
            size,
            modified_at: "now".to_string(),
            details: Some(json!({ "format": format })),
        }
    }

    #[test]
    fn blocks_cloud_and_non_gguf_models() {
        assert!(is_strictly_local_model(&model(
            "qwen3:4b", 2_000_000, "gguf"
        )));
        assert!(!is_strictly_local_model(&model(
            "gpt-oss:120b-cloud",
            2_000_000,
            "gguf"
        )));
        assert!(!is_strictly_local_model(&model("remote", 0, "gguf")));
        assert!(!is_strictly_local_model(&model(
            "unknown", 2_000_000, "remote"
        )));
    }

    #[test]
    fn backend_trims_old_context() {
        let messages = (0..20)
            .map(|_| ChatMessage {
                role: "user".to_string(),
                content: "x".repeat(1000),
            })
            .collect::<Vec<_>>();
        let selected = trim_messages(&messages, 4096, 768);
        assert!(!selected.is_empty());
        assert!(selected.len() < messages.len());
    }
}
