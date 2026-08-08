use super::{
    audit::append_audit,
    browser::validate_public_https,
    pc::confirm_native_action,
    policy::enforce_capability,
    storage::{redact_db, AppState},
};
use aes_gcm::{
    aead::{KeyInit, OsRng},
    Aes256Gcm,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use uuid::Uuid;

const PAIRING_AAD: &[u8] = b"aura-remote-pairing-v1|default";
const CLOCK_SKEW_SECONDS: i64 = 120;
const NONCE_RETENTION_SECONDS: i64 = 86_400;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    paired: bool,
    enabled: bool,
    device_name: String,
    relay_host: String,
    transport: &'static str,
    inbound_port: bool,
    allowed_commands: Vec<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingResult {
    paired: bool,
    pairing_secret: String,
    device_name: String,
    relay_host: String,
    warning: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEnvelope {
    version: u8,
    id: String,
    issued_at: i64,
    nonce: String,
    command: String,
    payload_json: String,
    signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedRemoteCommand {
    id: String,
    command: String,
    payload_json: String,
    policy: &'static str,
    executable: bool,
    reason: &'static str,
}

#[tauri::command]
pub fn remote_status(state: State<'_, AppState>) -> Result<RemoteStatus, String> {
    enforce_capability("remote.inspect", "allow")?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let row = db
        .query_row(
            "SELECT device_name,relay_url,enabled FROM remote_pairing WHERE id='default'",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(redact_db)?;
    let (paired, enabled, device_name, relay_host) = if let Some((name, url, enabled)) = row {
        let host = reqwest::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();
        (true, enabled == 1, name, host)
    } else {
        (false, false, String::new(), String::new())
    };
    Ok(RemoteStatus {
        paired,
        enabled,
        device_name,
        relay_host,
        transport: "outbound-https-polling",
        inbound_port: false,
        allowed_commands: allowed_commands(),
    })
}

#[tauri::command]
pub fn remote_pair(
    device_name: String,
    relay_url: String,
    state: State<'_, AppState>,
) -> Result<PairingResult, String> {
    enforce_capability("remote.pair", "confirm")?;
    if device_name.trim().is_empty() || device_name.len() > 80 {
        return Err("Nome dispositivo remoto non valido.".to_string());
    }
    let relay = validate_public_https(&relay_url)?;
    let host = relay.host_str().unwrap_or("relay esterno").to_string();
    if !confirm_native_action(
        "Conferma associazione Alexa",
        &format!("Associare AURA a '{device_name}' tramite {host}? Il segreto sarà mostrato una sola volta."),
    ) {
        return Err("Associazione remota annullata.".to_string());
    }
    let secret = Aes256Gcm::generate_key(&mut OsRng);
    let encoded = URL_SAFE_NO_PAD.encode(secret.as_slice());
    let (cipher, nonce) = state.crypto.encrypt_with_aad(&encoded, PAIRING_AAD)?;
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    db.execute(
        "INSERT INTO remote_pairing(id,device_name,relay_url,secret_cipher,secret_nonce,enabled,created_at)
         VALUES('default',?1,?2,?3,?4,1,datetime('now'))
         ON CONFLICT(id) DO UPDATE SET device_name=excluded.device_name,relay_url=excluded.relay_url,
         secret_cipher=excluded.secret_cipher,secret_nonce=excluded.secret_nonce,enabled=1,created_at=excluded.created_at",
        params![device_name, relay.to_string(), cipher, nonce],
    ).map_err(redact_db)?;
    db.execute("DELETE FROM remote_nonces", [])
        .map_err(redact_db)?;
    append_audit(
        &state,
        "remote-pair",
        "remote.paired",
        "confirm",
        "Dispositivo vocale remoto associato",
        &serde_json::json!({"host":host,"secretLogged":false,"inboundPort":false}).to_string(),
    )?;
    Ok(PairingResult {
        paired: true,
        pairing_secret: encoded,
        device_name,
        relay_host: host,
        warning: "Copia il segreto nel plugin Alexa ora: AURA non lo mostrerà di nuovo.",
    })
}

#[tauri::command]
pub fn remote_unpair(state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("remote.unpair", "confirm")?;
    if !confirm_native_action(
        "Conferma revoca Alexa",
        "Revocare l'associazione Alexa e invalidare immediatamente il segreto?",
    ) {
        return Ok(false);
    }
    let db = state
        .database
        .lock()
        .map_err(|_| "Database occupato.".to_string())?;
    let changed = db
        .execute("DELETE FROM remote_pairing WHERE id='default'", [])
        .map_err(redact_db)?
        > 0;
    db.execute("DELETE FROM remote_nonces", [])
        .map_err(redact_db)?;
    append_audit(
        &state,
        "remote-unpair",
        "remote.unpaired",
        "confirm",
        "Associazione vocale remota revocata",
        r#"{"secretLogged":false}"#,
    )?;
    Ok(changed)
}

#[tauri::command]
pub fn remote_verify_envelope(
    envelope: RemoteEnvelope,
    state: State<'_, AppState>,
) -> Result<VerifiedRemoteCommand, String> {
    enforce_capability("remote.command", "confirm")?;
    validate_envelope_shape(&envelope)?;
    let now = unix_now()?;
    if now.abs_diff(envelope.issued_at) > CLOCK_SKEW_SECONDS as u64 {
        return Err("Comando remoto scaduto o con orologio non valido.".to_string());
    }
    let (secret_cipher, secret_nonce, enabled) = {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.query_row(
            "SELECT secret_cipher,secret_nonce,enabled FROM remote_pairing WHERE id='default'",
            [],
            |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(redact_db)?
        .ok_or_else(|| "Nessun dispositivo remoto associato.".to_string())?
    };
    if enabled != 1 {
        return Err("Controllo remoto disabilitato.".to_string());
    }
    let encoded = state
        .crypto
        .decrypt_with_aad(&secret_cipher, &secret_nonce, PAIRING_AAD)?;
    let secret = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "Segreto pairing non valido.".to_string())?;
    verify_signature(&envelope, &secret)?;
    let policy = command_policy(&envelope.command)
        .ok_or_else(|| "Comando remoto non allowlistato.".to_string())?;
    {
        let db = state
            .database
            .lock()
            .map_err(|_| "Database occupato.".to_string())?;
        db.execute(
            "DELETE FROM remote_nonces WHERE seen_at<?1",
            [now - NONCE_RETENTION_SECONDS],
        )
        .map_err(redact_db)?;
        db.execute(
            "INSERT INTO remote_nonces(nonce,command_id,seen_at) VALUES(?1,?2,?3)",
            params![envelope.nonce, envelope.id, now],
        )
        .map_err(|_| "Replay remoto rilevato: nonce già utilizzato.".to_string())?;
    }
    append_audit(
        &state, &envelope.id, "remote.verified", policy,
        "Comando Alexa autenticato; esecuzione non ancora concessa",
        &serde_json::json!({"command":envelope.command,"payloadLogged":false,"remoteDoesNotBypassPolicy":true}).to_string(),
    )?;
    Ok(VerifiedRemoteCommand {
        id: envelope.id,
        command: envelope.command,
        payload_json: envelope.payload_json,
        policy,
        executable: policy == "allow",
        reason: if policy == "allow" {
            "Comando di sola lettura autenticato."
        } else {
            "Conferma locale o challenge vocale aggiuntiva richiesta."
        },
    })
}

fn allowed_commands() -> Vec<&'static str> {
    vec![
        "pc.status",
        "security.status",
        "app.open",
        "system.lock",
        "chat.ask",
    ]
}

fn command_policy(command: &str) -> Option<&'static str> {
    match command {
        "pc.status" | "security.status" => Some("allow"),
        "app.open" | "system.lock" | "chat.ask" => Some("confirm"),
        _ => None,
    }
}

fn validate_envelope_shape(e: &RemoteEnvelope) -> Result<(), String> {
    if e.version != 1
        || Uuid::parse_str(&e.id).is_err()
        || e.nonce.len() < 16
        || e.nonce.len() > 128
        || !e
            .nonce
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || e.command.len() > 64
        || e.payload_json.len() > 8192
        || serde_json::from_str::<serde_json::Value>(&e.payload_json).is_err()
        || e.signature.len() > 128
    {
        return Err("Busta comando remoto non valida.".to_string());
    }
    Ok(())
}

fn canonical(e: &RemoteEnvelope) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        e.version, e.id, e.issued_at, e.nonce, e.command, e.payload_json
    )
}

fn verify_signature(e: &RemoteEnvelope, secret: &[u8]) -> Result<(), String> {
    let signature = URL_SAFE_NO_PAD
        .decode(&e.signature)
        .map_err(|_| "Firma remota non valida.".to_string())?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret)
        .map_err(|_| "Chiave HMAC remota non valida.".to_string())?;
    mac.update(canonical(e).as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| "Firma comando remoto non autenticabile.".to_string())
}

fn unix_now() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .map_err(|_| "Orologio sistema non valido.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn signed(command: &str, secret: &[u8]) -> RemoteEnvelope {
        let mut envelope = RemoteEnvelope {
            version: 1,
            id: Uuid::new_v4().to_string(),
            issued_at: unix_now().unwrap(),
            nonce: "nonce_alexa_123456789".into(),
            command: command.into(),
            payload_json: "{}".into(),
            signature: String::new(),
        };
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret).unwrap();
        mac.update(canonical(&envelope).as_bytes());
        envelope.signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        envelope
    }
    #[test]
    fn verifies_valid_hmac_and_rejects_tampering() {
        let secret = [9_u8; 32];
        let mut envelope = signed("pc.status", &secret);
        assert!(verify_signature(&envelope, &secret).is_ok());
        envelope.command = "system.lock".into();
        assert!(verify_signature(&envelope, &secret).is_err());
    }
    #[test]
    fn remote_commands_never_bypass_mutating_policy() {
        assert_eq!(command_policy("pc.status"), Some("allow"));
        assert_eq!(command_policy("system.lock"), Some("confirm"));
        assert_eq!(command_policy("powershell"), None);
    }
}
