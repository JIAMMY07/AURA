use super::{
    audit::append_audit, pc::confirm_native_action, policy::enforce_capability, storage::AppState,
};
use reqwest::{redirect::Policy, Url};
use serde::Serialize;
use std::{
    net::{IpAddr, ToSocketAddrs},
    time::Duration,
};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "aura-controlled-browser";
const MAX_PAGE_BYTES: usize = 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    implemented: bool,
    interactive: bool,
    user_takeover: bool,
    network_policy: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPage {
    url: String,
    title: String,
    text: String,
    truncated: bool,
}

#[tauri::command]
pub fn browser_status(app: AppHandle) -> BrowserStatus {
    BrowserStatus {
        implemented: true,
        interactive: app.get_webview_window(LABEL).is_some(),
        user_takeover: true,
        network_policy: "HTTPS pubblico; localhost e indirizzi privati bloccati",
    }
}

#[tauri::command]
pub fn browser_open(
    url: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("browser.open", "confirm")?;
    let url = validate_public_https(&url)?;
    if !confirm_native_action("Conferma browser AURA", &format!("Aprire una finestra browser controllata su {}? Potrai prenderne il controllo in qualsiasi momento.", url.host_str().unwrap_or("sito esterno"))) { return Ok(false); }
    if let Some(window) = app.get_webview_window(LABEL) {
        window
            .navigate(url)
            .map_err(|_| "Navigazione WebView fallita.".to_string())?;
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url))
            .title("AURA Browser — controllo manuale sempre disponibile")
            .inner_size(1180.0, 780.0)
            .on_navigation(|destination| validate_public_https(destination.as_str()).is_ok())
            .build()
            .map_err(|_| "Creazione browser AURA fallita.".to_string())?;
    }
    append_audit(
        &state,
        "browser-open",
        "browser.opened",
        "confirm",
        "Browser controllato aperto",
        r#"{"urlLogged":false}"#,
    )?;
    Ok(true)
}

#[tauri::command]
pub fn browser_navigate(
    url: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("browser.navigate", "confirm")?;
    let url = validate_public_https(&url)?;
    if !confirm_native_action(
        "Conferma navigazione AURA",
        &format!(
            "Consentire la navigazione verso {}?",
            url.host_str().unwrap_or("sito esterno")
        ),
    ) {
        return Ok(false);
    }
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| "Apri prima il browser AURA.".to_string())?;
    window
        .navigate(url)
        .map_err(|_| "Navigazione WebView fallita.".to_string())?;
    append_audit(
        &state,
        "browser-navigate",
        "browser.navigated",
        "confirm",
        "Browser controllato navigato",
        r#"{"urlLogged":false}"#,
    )?;
    Ok(true)
}

#[tauri::command]
pub async fn browser_fetch(url: String, state: State<'_, AppState>) -> Result<BrowserPage, String> {
    enforce_capability("browser.inspect", "allow")?;
    let url = validate_public_https(&url)?;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .redirect(Policy::none())
        .build()
        .map_err(|_| "Client browser non disponibile.".to_string())?;
    let response = client
        .get(url.clone())
        .header("User-Agent", "AURA-Local/0.7")
        .send()
        .await
        .map_err(|_| "Pagina non raggiungibile.".to_string())?;
    if response.status().is_redirection() {
        return Err(
            "Redirect bloccato: autorizza esplicitamente la destinazione finale.".to_string(),
        );
    }
    if !response.status().is_success() {
        return Err(format!("Pagina restituisce HTTP {}.", response.status()));
    }
    let kind = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !(kind.contains("text/") || kind.contains("json") || kind.contains("xml")) {
        return Err("Tipo di contenuto non testuale bloccato.".to_string());
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_PAGE_BYTES as u64)
    {
        return Err("Pagina oltre il limite di 1 MiB.".to_string());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "Lettura pagina fallita.".to_string())?;
    let truncated = bytes.len() > MAX_PAGE_BYTES;
    let raw = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_PAGE_BYTES)]);
    let title = html_title(&raw).unwrap_or_else(|| url.host_str().unwrap_or("Pagina").to_string());
    let text = if kind.contains("html") {
        strip_html(&raw)
    } else {
        raw.to_string()
    };
    append_audit(
        &state,
        "browser-fetch",
        "browser.inspected",
        "allow",
        "Pagina pubblica letta",
        &serde_json::json!({"host":url.host_str(),"bytes":bytes.len().min(MAX_PAGE_BYTES)})
            .to_string(),
    )?;
    Ok(BrowserPage {
        url: url.to_string(),
        title,
        text: text.chars().take(40_000).collect(),
        truncated,
    })
}

#[tauri::command]
pub fn browser_close(app: AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window
            .close()
            .map_err(|_| "Chiusura browser fallita.".to_string())?;
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn validate_public_https(value: &str) -> Result<Url, String> {
    if value.len() > 2048 {
        return Err("URL troppo lungo.".to_string());
    }
    let url = Url::parse(value).map_err(|_| "URL non valido.".to_string())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(
            "Sono consentiti soltanto URL HTTPS senza credenziali incorporate.".to_string(),
        );
    }
    let host = url.host_str().ok_or_else(|| "Host assente.".to_string())?;
    if matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "localhost.localdomain"
    ) {
        return Err("Accesso alla rete locale bloccato.".to_string());
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|_| "DNS non risolvibile.".to_string())?;
    let mut found = false;
    for address in addresses {
        found = true;
        if !public_ip(address.ip()) {
            return Err(
                "SSRF bloccato: destinazione privata, locale o non instradabile.".to_string(),
            );
        }
    }
    if !found {
        return Err("Host senza indirizzi verificabili.".to_string());
    }
    Ok(url)
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_broadcast()
                || v.is_documentation()
                || v.is_unspecified()
                || v.is_multicast())
        }
        IpAddr::V6(v) => {
            !(v.is_loopback()
                || v.is_unspecified()
                || v.is_multicast()
                || v.is_unique_local()
                || v.is_unicast_link_local())
        }
    }
}

fn html_title(raw: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open = lower[start..].find('>')? + start + 1;
    let end = lower[open..].find("</title>")? + open;
    Some(strip_html(&raw[open..end]).chars().take(200).collect())
}

fn strip_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(40_000));
    let mut tag = false;
    let mut space = false;
    for c in raw.chars() {
        match c {
            '<' => tag = true,
            '>' => {
                tag = false;
                space = true;
            }
            _ if !tag => {
                if c.is_whitespace() {
                    space = true;
                } else {
                    if space && !out.is_empty() {
                        out.push(' ');
                    }
                    out.push(c);
                    space = false;
                }
            }
            _ => {}
        }
        if out.len() >= 80_000 {
            break;
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(test)]
mod tests {
    use super::{strip_html, validate_public_https};
    #[test]
    fn blocks_local_and_insecure_urls() {
        assert!(validate_public_https("http://example.com").is_err());
        assert!(validate_public_https("https://127.0.0.1/admin").is_err());
        assert!(validate_public_https("https://localhost").is_err());
    }
    #[test]
    fn extracts_readable_text() {
        assert_eq!(strip_html("<h1>AURA</h1><p>locale</p>"), "AURA locale");
    }
}
