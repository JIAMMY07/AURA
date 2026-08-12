use serde::Serialize;
use std::time::Duration;

const CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderDescriptor {
    id: String,
    display_name: String,
    api_kind: String,
    endpoint: Option<String>,
    configured: bool,
    active: bool,
    reachable: bool,
    local_only: bool,
    contract_version: u32,
    capabilities: Vec<String>,
    notes: String,
}

fn provider_catalog(ollama_reachable: bool) -> Vec<AiProviderDescriptor> {
    vec![
        AiProviderDescriptor {
            id: "ollama".into(),
            display_name: "Ollama locale".into(),
            api_kind: "ollama".into(),
            endpoint: Some("http://127.0.0.1:11434".into()),
            configured: true,
            active: true,
            reachable: ollama_reachable,
            local_only: true,
            contract_version: CONTRACT_VERSION,
            capabilities: vec!["chat-stream".into(), "models".into(), "vision".into(), "embeddings".into()],
            notes: "Provider attivo. Chat e funzioni specialistiche AURA usano oggi questo runtime.".into(),
        },
        AiProviderDescriptor {
            id: "llama-cpp".into(),
            display_name: "llama.cpp server".into(),
            api_kind: "openai-compatible".into(),
            endpoint: Some("http://127.0.0.1:8080/v1".into()),
            configured: false,
            active: false,
            reachable: false,
            local_only: true,
            contract_version: CONTRACT_VERSION,
            capabilities: vec!["chat-stream".into(), "models".into()],
            notes: "Predisposto tramite il contratto OpenAI-compatible; runtime non installato né attivato.".into(),
        },
        AiProviderDescriptor {
            id: "local-ai".into(),
            display_name: "LocalAI".into(),
            api_kind: "openai-compatible".into(),
            endpoint: Some("http://127.0.0.1:8081/v1".into()),
            configured: false,
            active: false,
            reachable: false,
            local_only: true,
            contract_version: CONTRACT_VERSION,
            capabilities: vec!["chat-stream".into(), "models".into(), "embeddings".into()],
            notes: "Predisposto; installazione, verifica del runtime e scelta esplicita restano necessarie.".into(),
        },
        AiProviderDescriptor {
            id: "openai-compatible-local".into(),
            display_name: "Endpoint locale compatibile OpenAI".into(),
            api_kind: "openai-compatible".into(),
            endpoint: None,
            configured: false,
            active: false,
            reachable: false,
            local_only: true,
            contract_version: CONTRACT_VERSION,
            capabilities: vec!["chat-stream".into(), "models".into()],
            notes: "Slot generico data-only. Richiede endpoint loopback, adapter verificato e test di sicurezza.".into(),
        },
    ]
}

async fn ollama_reachable() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    client
        .get("http://127.0.0.1:11434/api/tags")
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn ai_provider_list() -> Vec<AiProviderDescriptor> {
    provider_catalog(ollama_reachable().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_has_unique_ids_and_one_active_provider() {
        let catalog = provider_catalog(true);
        let ids: HashSet<_> = catalog.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids.len(), catalog.len());
        assert_eq!(catalog.iter().filter(|item| item.active).count(), 1);
        assert!(catalog
            .iter()
            .all(|item| item.contract_version == CONTRACT_VERSION));
    }

    #[test]
    fn future_providers_are_fail_closed_and_local() {
        for provider in provider_catalog(false)
            .into_iter()
            .filter(|item| item.id != "ollama")
        {
            assert!(!provider.configured);
            assert!(!provider.active);
            assert!(!provider.reachable);
            assert!(provider.local_only);
        }
    }
}
