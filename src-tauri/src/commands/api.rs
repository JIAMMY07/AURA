use super::{
    audit::append_audit,
    browser::{resolve_public_https, validate_public_https},
    pc::confirm_native_action,
    policy::enforce_capability,
    storage::{redact_db, AppState},
};
use reqwest::{redirect::Policy, Method};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;
use uuid::Uuid;
use zeroize::Zeroizing;

const MAX_RESPONSE: usize = 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConnectorView {
    id: String,
    name: String,
    base_url: String,
    auth_kind: String,
    auth_header: String,
    has_secret: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConnectorInput {
    id: Option<String>,
    name: String,
    base_url: String,
    auth_kind: String,
    auth_header: String,
    secret: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCallInput {
    connector_id: String,
    method: String,
    path: String,
    body_json: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCallResult {
    status: u16,
    content_type: String,
    body: String,
    truncated: bool,
}

#[tauri::command]
pub fn api_list_connectors(state: State<'_, AppState>) -> Result<Vec<ApiConnectorView>, String> {
    enforce_capability("api.inspect", "allow")?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let mut statement = db.prepare("SELECT id,name,base_url,auth_kind,auth_header,length(secret_cipher)>16 FROM api_connectors ORDER BY name").map_err(redact_db)?;
    let rows = statement
        .query_map([], |r| {
            Ok(ApiConnectorView {
                id: r.get(0)?,
                name: r.get(1)?,
                base_url: r.get(2)?,
                auth_kind: r.get(3)?,
                auth_header: r.get(4)?,
                has_secret: r.get(5)?,
            })
        })
        .map_err(redact_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(redact_db)?;
    Ok(rows)
}

#[tauri::command]
pub fn api_save_connector(
    input: ApiConnectorInput,
    state: State<'_, AppState>,
) -> Result<String, String> {
    enforce_capability("api.configure", "confirm")?;
    validate_connector(&input)?;
    let base = validate_public_https(&input.base_url)?;
    if !confirm_native_action(
        "Conferma connettore API",
        &format!(
            "Salvare cifrato il connettore '{}' per {}?",
            input.name,
            base.host_str().unwrap_or("host esterno")
        ),
    ) {
        return Err("Salvataggio connettore annullato.".to_string());
    }
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let aad = format!("api-connector-v1|{id}|{}", input.auth_kind);
    let (cipher, nonce) = state
        .crypto
        .encrypt_with_aad(&input.secret, aad.as_bytes())?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    db.execute("INSERT INTO api_connectors(id,name,base_url,auth_kind,auth_header,secret_cipher,secret_nonce,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,datetime('now')) ON CONFLICT(id) DO UPDATE SET name=excluded.name,base_url=excluded.base_url,auth_kind=excluded.auth_kind,auth_header=excluded.auth_header,secret_cipher=excluded.secret_cipher,secret_nonce=excluded.secret_nonce", params![id,input.name,base.to_string(),input.auth_kind,input.auth_header,cipher,nonce]).map_err(redact_db)?;
    append_audit(
        &state,
        "api-configure",
        "api.configured",
        "confirm",
        "Connettore API cifrato salvato",
        &serde_json::json!({"id":id,"host":base.host_str(),"secretLogged":false}).to_string(),
    )?;
    Ok(id)
}

#[tauri::command]
pub fn api_delete_connector(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("api.configure", "confirm")?;
    if Uuid::parse_str(&id).is_err() {
        return Err("ID connettore non valido.".to_string());
    }
    if !confirm_native_action(
        "Conferma eliminazione API",
        "Eliminare il connettore e la credenziale cifrata?",
    ) {
        return Ok(false);
    }
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let changed = db
        .execute("DELETE FROM api_connectors WHERE id=?1", [&id])
        .map_err(redact_db)?
        > 0;
    append_audit(
        &state,
        "api-delete",
        "api.deleted",
        "confirm",
        "Connettore API eliminato",
        &serde_json::json!({"id":id}).to_string(),
    )?;
    Ok(changed)
}

#[tauri::command]
pub async fn api_call(
    input: ApiCallInput,
    state: State<'_, AppState>,
) -> Result<ApiCallResult, String> {
    enforce_capability("api.call", "confirm")?;
    validate_call(&input)?;
    let (base_url, auth_kind, auth_header, cipher, nonce) = {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.query_row("SELECT base_url,auth_kind,auth_header,secret_cipher,secret_nonce FROM api_connectors WHERE id=?1", [&input.connector_id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Vec<u8>>(3)?,r.get::<_,Vec<u8>>(4)?))).optional().map_err(redact_db)?.ok_or_else(|| "Connettore API non trovato.".to_string())?
    };
    let base = validate_public_https(&base_url)?;
    let mut target = base
        .join(input.path.trim_start_matches('/'))
        .map_err(|_| "Percorso API non valido.".to_string())?;
    if target.origin() != base.origin() {
        return Err("Il percorso API non può cambiare host.".to_string());
    }
    if auth_kind == "query" {
        target
            .query_pairs_mut()
            .append_pair(&auth_header, "__AURA_SECRET_PLACEHOLDER__");
    }
    let secret = Zeroizing::new(state.crypto.decrypt_with_aad(
        &cipher,
        &nonce,
        format!("api-connector-v1|{}|{auth_kind}", input.connector_id).as_bytes(),
    )?);
    if auth_kind == "query" {
        let retained = target
            .query_pairs()
            .filter(|(key, value)| {
                !(key.as_ref() == auth_header && value == "__AURA_SECRET_PLACEHOLDER__")
            })
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        target.set_query(None);
        target
            .query_pairs_mut()
            .extend_pairs(retained)
            .append_pair(&auth_header, secret.as_str());
    }
    let (target, target_host, target_addresses) = resolve_public_https(target.as_str())?;
    if !confirm_native_action(
        "Conferma chiamata API",
        &format!(
            "Inviare {} a {}? Il corpo e la credenziale non saranno registrati nell'audit.",
            input.method,
            target.host_str().unwrap_or("host esterno")
        ),
    ) {
        return Err("Chiamata API annullata.".to_string());
    }
    let method = Method::from_bytes(input.method.as_bytes())
        .map_err(|_| "Metodo HTTP non valido.".to_string())?;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .redirect(Policy::none())
        .resolve_to_addrs(&target_host, &target_addresses)
        .build()
        .map_err(|_| "Client API non disponibile.".to_string())?;
    let mut request = client
        .request(method, target.clone())
        .header("Accept", "application/json, text/plain;q=0.9");
    if auth_kind == "bearer" {
        request = request.bearer_auth(secret.as_str());
    } else if auth_kind == "header" {
        request = request.header(&auth_header, secret.as_str());
    } else if auth_kind == "basic" {
        let (username, password) = secret
            .split_once(':')
            .ok_or_else(|| "Credenziale Basic non valida: usa utente:password.".to_string())?;
        request = request.basic_auth(username, Some(password));
    }
    if !input.body_json.trim().is_empty() {
        let value: serde_json::Value = serde_json::from_str(&input.body_json)
            .map_err(|_| "Body JSON non valido.".to_string())?;
        request = request.json(&value);
    }
    let response = request
        .send()
        .await
        .map_err(|_| "Chiamata API fallita.".to_string())?;
    if response.status().is_redirection() {
        return Err("Redirect API bloccato.".to_string());
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE as u64)
    {
        return Err("Risposta API oltre 1 MiB.".to_string());
    }
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "Lettura risposta API fallita.".to_string())?;
    let truncated = bytes.len() > MAX_RESPONSE;
    let body = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_RESPONSE)]).to_string();
    append_audit(&state, "api-call", "api.called", "confirm", "Chiamata API completata", &serde_json::json!({"connectorId":input.connector_id,"host":target.host_str(),"status":status,"bodyLogged":false}).to_string())?;
    Ok(ApiCallResult {
        status,
        content_type,
        body,
        truncated,
    })
}

fn validate_connector(input: &ApiConnectorInput) -> Result<(), String> {
    if input.name.trim().is_empty()
        || input.name.len() > 80
        || !matches!(
            input.auth_kind.as_str(),
            "none" | "bearer" | "header" | "basic" | "query"
        )
        || input.secret.len() > 8192
    {
        return Err("Configurazione connettore fuori dai limiti.".to_string());
    }
    let auth_name_valid = match input.auth_kind.as_str() {
        "header" => input
            .auth_header
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "query" => input
            .auth_header
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')),
        _ => true,
    };
    if matches!(input.auth_kind.as_str(), "header" | "query")
        && (input.auth_header.is_empty() || input.auth_header.len() > 80 || !auth_name_valid)
    {
        return Err("Nome header API non valido.".to_string());
    }
    if input.auth_kind != "none" && input.secret.is_empty() {
        return Err("Credenziale API mancante.".to_string());
    }
    if input.auth_kind == "basic" && !input.secret.contains(':') {
        return Err("Per Basic usa il formato utente:password.".to_string());
    }
    Ok(())
}
fn validate_call(input: &ApiCallInput) -> Result<(), String> {
    if Uuid::parse_str(&input.connector_id).is_err()
        || !matches!(
            input.method.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
        )
        || input.path.len() > 2048
        || input.path.starts_with("//")
        || input.body_json.len() > 256 * 1024
    {
        return Err("Chiamata API fuori dai limiti.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_call, validate_connector, ApiCallInput, ApiConnectorInput};
    #[test]
    fn validates_connector_and_call_bounds() {
        let bad = ApiConnectorInput {
            id: None,
            name: "x".into(),
            base_url: "http://local".into(),
            auth_kind: "shell".into(),
            auth_header: "".into(),
            secret: "".into(),
        };
        assert!(validate_connector(&bad).is_err());
        let call = ApiCallInput {
            connector_id: "bad".into(),
            method: "CONNECT".into(),
            path: "//evil".into(),
            body_json: "".into(),
        };
        assert!(validate_call(&call).is_err());
        let basic = ApiConnectorInput {
            id: None,
            name: "Basic".into(),
            base_url: "https://example.com/api/".into(),
            auth_kind: "basic".into(),
            auth_header: "Authorization".into(),
            secret: "utente:password".into(),
        };
        assert!(validate_connector(&basic).is_ok());
        let query = ApiConnectorInput {
            id: None,
            name: "Query key".into(),
            base_url: "https://example.com/api/".into(),
            auth_kind: "query".into(),
            auth_header: "api_key".into(),
            secret: "segreto".into(),
        };
        assert!(validate_connector(&query).is_ok());
    }
}
