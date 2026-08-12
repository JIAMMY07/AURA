use super::{
    audit::append_audit, pc::confirm_native_action, policy::enforce_capability, storage::AppState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    process::{Command, Stdio},
    time::Duration,
};
use tauri::State;
use wait_timeout::ChildExt;

const STATUS_SCRIPT: &str = r#"
$ErrorActionPreference='SilentlyContinue'
$mp=Get-MpComputerStatus
$fw=Get-NetFirewallProfile
$uac=(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System').EnableLUA
try{$secureBoot=Confirm-SecureBootUEFI}catch{$secureBoot=$null}
$wu=Get-Service wuauserv
try{$bitlocker=Get-BitLockerVolume -MountPoint C:}catch{$bitlocker=$null}
try{$tpm=Get-Tpm}catch{$tpm=$null}
$signature=Get-AuthenticodeSignature -LiteralPath $env:AURA_SECURITY_EXE
[pscustomobject]@{
 DefenderAvailable=[bool]$mp
 DefenderService=[bool]$mp.AMServiceEnabled
 Realtime=[bool]$mp.RealTimeProtectionEnabled
 Antivirus=[bool]$mp.AntivirusEnabled
 Antispyware=[bool]$mp.AntispywareEnabled
 BehaviorMonitor=[bool]$mp.BehaviorMonitorEnabled
 Ioav=[bool]$mp.IoavProtectionEnabled
 Nis=[bool]$mp.NISEnabled
 TamperProtected=[bool]$mp.IsTamperProtected
 CloudProtection=if($mp){[int]$mp.MAPSReporting}else{$null}
 SignaturesDays=[int]$mp.AntivirusSignatureAge
 QuickScanAge=[int]$mp.QuickScanAge
 FirewallDomain=[bool](($fw|Where-Object Name -eq Domain).Enabled)
 FirewallPrivate=[bool](($fw|Where-Object Name -eq Private).Enabled)
 FirewallPublic=[bool](($fw|Where-Object Name -eq Public).Enabled)
 Uac=($uac -eq 1)
 SecureBoot=$secureBoot
 WindowsUpdateDisabled=($wu.StartType -eq 'Disabled')
 BitLocker=if($bitlocker){[string]$bitlocker.ProtectionStatus}else{$null}
 Tpm=if($tpm){[bool]($tpm.TpmPresent -and $tpm.TpmReady)}else{$null}
 SignatureStatus=[string]$signature.Status
 Signer=if($signature.SignerCertificate){[string]$signature.SignerCertificate.Subject}else{''}
 Timestamped=[bool]$signature.TimeStamperCertificate
} | ConvertTo-Json -Compress
"#;

const INCIDENT_SCRIPT: &str = r#"
$ErrorActionPreference='SilentlyContinue'
$mp=Get-MpComputerStatus
$fw=@(Get-NetFirewallProfile)
$threats=@(Get-MpThreat)
$active=@($threats|Where-Object {$_.IsActive})
$recent=@(Get-MpThreatDetection|Where-Object {$_.InitialDetectionTime -ge (Get-Date).AddDays(-7)})
$tcp=@(Get-NetTCPConnection)
$established=@($tcp|Where-Object {$_.State -eq 'Established'})
$listeners=@($tcp|Where-Object {$_.State -eq 'Listen'})
$signature=Get-AuthenticodeSignature -LiteralPath $env:AURA_SECURITY_EXE
[pscustomobject]@{
 DefenderAvailable=[bool]$mp
 Realtime=[bool]$mp.RealTimeProtectionEnabled
 NetworkInspection=[bool]$mp.NISEnabled
 FirewallAll=[bool]($fw.Count -ge 3 -and @($fw|Where-Object {-not $_.Enabled}).Count -eq 0)
 ActiveThreats=[int]$active.Count
 RecentDetections=[int]$recent.Count
 EstablishedConnections=[int]$established.Count
 ListeningEndpoints=[int]$listeners.Count
 SignatureStatus=[string]$signature.Status
} | ConvertTo-Json -Compress
"#;

const REMEDIATE_SCRIPT: &str = r#"
$ErrorActionPreference='Stop'
$before=@(Get-MpThreat|Where-Object {$_.IsActive}).Count
if($before -gt 0){Remove-MpThreat}
Start-MpScan -ScanType QuickScan
$after=@(Get-MpThreat|Where-Object {$_.IsActive}).Count
[pscustomobject]@{Before=[int]$before;After=[int]$after;ScanCompleted=$true} | ConvertTo-Json -Compress
"#;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawStatus {
    defender_available: bool,
    defender_service: bool,
    realtime: bool,
    antivirus: bool,
    antispyware: bool,
    behavior_monitor: bool,
    ioav: bool,
    nis: bool,
    tamper_protected: bool,
    cloud_protection: Option<i64>,
    signatures_days: i64,
    quick_scan_age: i64,
    firewall_domain: bool,
    firewall_private: bool,
    firewall_public: bool,
    uac: bool,
    secure_boot: Option<bool>,
    windows_update_disabled: bool,
    bit_locker: Option<String>,
    tpm: Option<bool>,
    signature_status: String,
    signer: String,
    timestamped: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawIncidentStatus {
    defender_available: bool,
    realtime: bool,
    network_inspection: bool,
    firewall_all: bool,
    active_threats: u32,
    recent_detections: u32,
    established_connections: u32,
    listening_endpoints: u32,
    signature_status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentStatus {
    severity: &'static str,
    active_threats: u32,
    recent_detections: u32,
    established_connections: u32,
    listening_endpoints: u32,
    defender_healthy: bool,
    firewall_healthy: bool,
    aura_integrity: &'static str,
    automatic_containment: bool,
    summary: String,
    recommendations: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawRemediationResult {
    before: u32,
    after: u32,
    scan_completed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemediationResult {
    confirmed: bool,
    threats_before: u32,
    threats_after: u32,
    scan_completed: bool,
    contained: bool,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityCheck {
    id: &'static str,
    label: &'static str,
    status: &'static str,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityCenterStatus {
    module_complete: bool,
    coverage_score: u8,
    posture_score: u8,
    checks: Vec<SecurityCheck>,
    executable_sha256: String,
    public_signature: bool,
    local_signature: bool,
    signature_status: String,
    signer: String,
    timestamped: bool,
    recommendations: Vec<String>,
}

#[tauri::command]
pub async fn security_center_status(
    state: State<'_, AppState>,
) -> Result<SecurityCenterStatus, String> {
    enforce_capability("security.inspect", "allow")?;
    let executable =
        std::env::current_exe().map_err(|_| "Percorso eseguibile non disponibile.".to_string())?;
    let executable_for_script = executable.clone();
    let raw = tauri::async_runtime::spawn_blocking(move || {
        run_powershell(
            STATUS_SCRIPT,
            Duration::from_secs(30),
            Some((&executable_for_script, "AURA_SECURITY_EXE")),
        )
    })
    .await
    .map_err(|_| "Worker Security Center terminato.".to_string())??;
    let status: RawStatus = serde_json::from_str(raw.trim())
        .map_err(|_| "Stato sicurezza Windows non interpretabile.".to_string())?;
    let bytes = fs::read(executable).map_err(|_| "Hash eseguibile non calcolabile.".to_string())?;
    let hash = hex::encode_upper(Sha256::digest(bytes));
    let mut score = 0u8;
    let mut checks = Vec::new();
    push_bool(
        &mut checks,
        &mut score,
        "defender",
        "Microsoft Defender",
        status.defender_available && status.defender_service && status.antivirus,
        12,
        "servizio antivirus e motore",
    );
    push_bool(
        &mut checks,
        &mut score,
        "realtime",
        "Protezione in tempo reale",
        status.realtime,
        10,
        "monitoraggio Defender",
    );
    push_bool(
        &mut checks,
        &mut score,
        "defender-stack",
        "Protezioni Defender estese",
        status.antispyware && status.behavior_monitor && status.ioav && status.nis,
        8,
        "antispyware, comportamento, download e rete",
    );
    push_bool(
        &mut checks,
        &mut score,
        "tamper-protection",
        "Protezione antimanomissione",
        status.tamper_protected,
        8,
        "Tamper Protection Defender",
    );
    push_optional(
        &mut checks,
        &mut score,
        "cloud-protection",
        "Protezione cloud Defender",
        status.cloud_protection.map(|value| value > 0),
        5,
        "segnalazione MAPS attiva",
        "protezione cloud disattivata",
    );
    push_bool(
        &mut checks,
        &mut score,
        "firewall",
        "Windows Firewall",
        status.firewall_domain && status.firewall_private && status.firewall_public,
        10,
        "profili Domain, Private e Public",
    );
    push_bool(
        &mut checks,
        &mut score,
        "uac",
        "Controllo account utente",
        status.uac,
        8,
        "EnableLUA",
    );
    let signatures_ok = status.signatures_days <= 3;
    if signatures_ok {
        score += 8;
    }
    checks.push(SecurityCheck {
        id: "definitions",
        label: "Definizioni antivirus",
        status: if signatures_ok { "healthy" } else { "warning" },
        detail: format!("età: {} giorni", status.signatures_days),
    });
    let scan_ok = status.quick_scan_age <= 7;
    if scan_ok {
        score += 6;
    }
    checks.push(SecurityCheck {
        id: "scan",
        label: "Scansione rapida",
        status: if scan_ok { "healthy" } else { "warning" },
        detail: format!("ultima: {} giorni", status.quick_scan_age),
    });
    push_optional(
        &mut checks,
        &mut score,
        "secure-boot",
        "Secure Boot",
        status.secure_boot,
        8,
        "attivo",
        "disattivato",
    );
    let update_ok = !status.windows_update_disabled;
    if update_ok {
        score += 5;
    }
    checks.push(SecurityCheck {
        id: "windows-update",
        label: "Windows Update",
        status: if update_ok { "healthy" } else { "warning" },
        detail: if update_ok {
            "servizio non disabilitato"
        } else {
            "servizio disabilitato"
        }
        .into(),
    });
    let bitlocker_ok = status
        .bit_locker
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case("On") || v == "1");
    push_optional(
        &mut checks,
        &mut score,
        "bitlocker",
        "Cifratura unità",
        status.bit_locker.as_ref().map(|_| bitlocker_ok),
        6,
        "protezione attiva",
        "protezione disattivata",
    );
    push_optional(
        &mut checks,
        &mut score,
        "tpm",
        "TPM",
        status.tpm,
        6,
        "presente e pronto",
        "assente o non pronto",
    );
    let public_signature = status.signature_status.eq_ignore_ascii_case("Valid")
        && !status.signer.contains("AURA Local Development")
        && status.timestamped;
    let local_signature = !status.signer.is_empty()
        && !matches!(
            status.signature_status.as_str(),
            "NotSigned" | "HashMismatch" | ""
        );
    let recommendations = checks
        .iter()
        .filter(|check| check.status != "healthy")
        .map(|check| format!("{}: {}", check.label, check.detail))
        .collect::<Vec<_>>();
    let module_complete = checks.len() == 13 && !hash.is_empty();
    append_audit(&state,"security-center-status","security.inspected","allow","Postura sicurezza Windows verificata",&serde_json::json!({"score":score,"publicSignature":public_signature,"hash":hash,"sensitiveLogged":false}).to_string())?;
    Ok(SecurityCenterStatus {
        module_complete,
        coverage_score: if module_complete { 100 } else { 0 },
        posture_score: score,
        checks,
        executable_sha256: hash,
        public_signature,
        local_signature,
        signature_status: status.signature_status,
        signer: status.signer,
        timestamped: status.timestamped,
        recommendations,
    })
}

#[tauri::command]
pub async fn security_incident_status(
    state: State<'_, AppState>,
) -> Result<IncidentStatus, String> {
    enforce_capability("security.incident.inspect", "allow")?;
    let executable =
        std::env::current_exe().map_err(|_| "Percorso eseguibile non disponibile.".to_string())?;
    let raw = tauri::async_runtime::spawn_blocking(move || {
        run_powershell(
            INCIDENT_SCRIPT,
            Duration::from_secs(30),
            Some((&executable, "AURA_SECURITY_EXE")),
        )
    })
    .await
    .map_err(|_| "Worker incident response terminato.".to_string())??;
    let status: RawIncidentStatus = serde_json::from_str(raw.trim())
        .map_err(|_| "Stato incident response non interpretabile.".to_string())?;

    let defender_healthy =
        status.defender_available && status.realtime && status.network_inspection;
    let aura_integrity = match status.signature_status.as_str() {
        "Valid" => "verified",
        "HashMismatch" => "compromised",
        _ => "unverified",
    };
    let severity = classify_incident(
        status.active_threats,
        status.recent_detections,
        defender_healthy,
        status.firewall_all,
        aura_integrity,
    );
    let mut recommendations = Vec::new();
    if status.active_threats > 0 {
        recommendations.push(
            "Avviare la remediation Defender con conferma e verificare nuovamente le minacce attive."
                .to_string(),
        );
    }
    if !defender_healthy {
        recommendations.push(
            "Ripristinare protezione in tempo reale e Network Inspection tramite Sicurezza di Windows."
                .to_string(),
        );
    }
    if !status.firewall_all {
        recommendations.push("Verificare i tre profili Windows Firewall.".to_string());
    }
    if aura_integrity != "verified" {
        recommendations.push(
            "Verificare firma e provenienza dell'eseguibile AURA prima di concedere privilegi."
                .to_string(),
        );
    }
    if status.recent_detections > 0 {
        recommendations.push(
            "Esaminare la cronologia Defender e preservare le evidenze prima della rimozione."
                .to_string(),
        );
    }
    let summary = format!(
        "Severita {severity}: {} minacce attive, {} rilevamenti negli ultimi 7 giorni. Le {} connessioni stabilite e i {} endpoint in ascolto sono telemetria, non prova autonoma di attacco.",
        status.active_threats,
        status.recent_detections,
        status.established_connections,
        status.listening_endpoints
    );
    append_audit(
        &state,
        "security-incident-status",
        "security.incident.inspected",
        "allow",
        "Triage incident response completato",
        &serde_json::json!({
            "severity": severity,
            "activeThreats": status.active_threats,
            "recentDetections": status.recent_detections,
            "networkAddressesLogged": false
        })
        .to_string(),
    )?;
    Ok(IncidentStatus {
        severity,
        active_threats: status.active_threats,
        recent_detections: status.recent_detections,
        established_connections: status.established_connections,
        listening_endpoints: status.listening_endpoints,
        defender_healthy,
        firewall_healthy: status.firewall_all,
        aura_integrity,
        automatic_containment: false,
        summary,
        recommendations,
    })
}

#[tauri::command]
pub async fn security_remediate_threats(
    state: State<'_, AppState>,
) -> Result<RemediationResult, String> {
    enforce_capability("security.remediate", "confirm")?;
    if !confirm_native_action(
        "Conferma contenimento Defender",
        "Rimuovere tramite Microsoft Defender le minacce attive rilevate e avviare una scansione rapida? L'operazione sara verificata al termine.",
    ) {
        return Ok(RemediationResult {
            confirmed: false,
            threats_before: 0,
            threats_after: 0,
            scan_completed: false,
            contained: false,
            message: "Contenimento annullato dall'utente.".to_string(),
        });
    }
    let raw = tauri::async_runtime::spawn_blocking(|| {
        run_powershell(REMEDIATE_SCRIPT, Duration::from_secs(900), None)
    })
    .await
    .map_err(|_| "Worker remediation Defender terminato.".to_string())??;
    let result: RawRemediationResult = serde_json::from_str(raw.trim())
        .map_err(|_| "Esito remediation non interpretabile.".to_string())?;
    let contained = result.scan_completed && result.after == 0;
    let message = if contained {
        "Scansione completata e nessuna minaccia attiva residua rilevata."
    } else {
        "La remediation non ha eliminato tutte le minacce: isolare manualmente il PC dalla rete e usare Sicurezza di Windows o assistenza qualificata."
    }
    .to_string();
    append_audit(
        &state,
        "security-remediate-threats",
        "security.incident.remediated",
        "confirm",
        "Remediation Defender verificata",
        &serde_json::json!({
            "before": result.before,
            "after": result.after,
            "scanCompleted": result.scan_completed,
            "contained": contained,
            "threatNamesLogged": false
        })
        .to_string(),
    )?;
    Ok(RemediationResult {
        confirmed: true,
        threats_before: result.before,
        threats_after: result.after,
        scan_completed: result.scan_completed,
        contained,
        message,
    })
}

#[tauri::command]
pub async fn security_quick_scan(state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("security.scan", "confirm")?;
    if !confirm_native_action(
        "Conferma scansione Defender",
        "Avviare una scansione rapida Microsoft Defender?",
    ) {
        return Ok(false);
    }
    tauri::async_runtime::spawn_blocking(|| {
        run_powershell(
            "Start-MpScan -ScanType QuickScan; '{\"started\":true}'",
            Duration::from_secs(900),
            None,
        )
    })
    .await
    .map_err(|_| "Worker Defender terminato.".to_string())??;
    append_audit(
        &state,
        "security-quick-scan",
        "security.scanned",
        "confirm",
        "Scansione rapida Defender completata",
        r#"{"threatDataLogged":false}"#,
    )?;
    Ok(true)
}

#[tauri::command]
pub async fn security_update_definitions(state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("security.update", "confirm")?;
    if !confirm_native_action(
        "Conferma aggiornamento Defender",
        "Aggiornare ora le definizioni Microsoft Defender tramite il canale configurato?",
    ) {
        return Ok(false);
    }
    tauri::async_runtime::spawn_blocking(|| {
        run_powershell(
            "Update-MpSignature; '{\"updated\":true}'",
            Duration::from_secs(300),
            None,
        )
    })
    .await
    .map_err(|_| "Worker aggiornamento terminato.".to_string())??;
    append_audit(
        &state,
        "security-update",
        "security.updated",
        "confirm",
        "Definizioni Defender aggiornate",
        r#"{"source":"configured-windows-channel"}"#,
    )?;
    Ok(true)
}

fn push_bool(
    checks: &mut Vec<SecurityCheck>,
    score: &mut u8,
    id: &'static str,
    label: &'static str,
    healthy: bool,
    weight: u8,
    detail: &'static str,
) {
    if healthy {
        *score += weight;
    }
    checks.push(SecurityCheck {
        id,
        label,
        status: if healthy { "healthy" } else { "warning" },
        detail: detail.into(),
    });
}

fn classify_incident(
    active_threats: u32,
    recent_detections: u32,
    defender_healthy: bool,
    firewall_healthy: bool,
    aura_integrity: &str,
) -> &'static str {
    if active_threats > 0 || aura_integrity == "compromised" {
        "critical"
    } else if !defender_healthy || !firewall_healthy {
        "high"
    } else if recent_detections > 0 || aura_integrity == "unverified" {
        "warning"
    } else {
        "healthy"
    }
}

#[allow(clippy::too_many_arguments)]
fn push_optional(
    checks: &mut Vec<SecurityCheck>,
    score: &mut u8,
    id: &'static str,
    label: &'static str,
    healthy: Option<bool>,
    weight: u8,
    healthy_detail: &'static str,
    warning_detail: &'static str,
) {
    if healthy == Some(true) {
        *score += weight;
    }
    checks.push(SecurityCheck {
        id,
        label,
        status: match healthy {
            Some(true) => "healthy",
            Some(false) => "warning",
            None => "unavailable",
        },
        detail: match healthy {
            Some(true) => healthy_detail,
            Some(false) => warning_detail,
            None => "lettura non disponibile nella sessione corrente",
        }
        .into(),
    });
}

fn run_powershell(
    script: &str,
    timeout: Duration,
    environment: Option<(&std::path::Path, &str)>,
) -> Result<String, String> {
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some((value, name)) = environment {
        command.env(name, value);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "PowerShell sicurezza non avviabile.".to_string())?;
    let _registration = super::task_control::register_child(child.id());
    use std::io::Write;
    child
        .stdin
        .take()
        .ok_or_else(|| "stdin PowerShell assente.".to_string())?
        .write_all(script.as_bytes())
        .map_err(|_| "Invio controllo sicurezza fallito.".to_string())?;
    match child
        .wait_timeout(timeout)
        .map_err(|_| "Attesa controllo sicurezza fallita.".to_string())?
    {
        Some(status) if status.success() => {
            let output = child
                .wait_with_output()
                .map_err(|_| "Lettura controllo sicurezza fallita.".to_string())?;
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Some(_) => Err("Controllo sicurezza Windows fallito.".to_string()),
        None => {
            let _ = child.kill();
            Err("Controllo sicurezza scaduto.".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_incident, push_bool};
    #[test]
    fn posture_weights_are_deterministic() {
        let mut checks = Vec::new();
        let mut score = 0;
        push_bool(&mut checks, &mut score, "x", "X", true, 20, "ok");
        push_bool(&mut checks, &mut score, "y", "Y", false, 10, "no");
        assert_eq!(score, 20);
        assert_eq!(checks.len(), 2);
    }

    #[test]
    fn incident_classification_is_fail_safe_and_deterministic() {
        assert_eq!(classify_incident(1, 0, true, true, "verified"), "critical");
        assert_eq!(
            classify_incident(0, 0, true, true, "compromised"),
            "critical"
        );
        assert_eq!(classify_incident(0, 0, false, true, "verified"), "high");
        assert_eq!(classify_incident(0, 2, true, true, "verified"), "warning");
        assert_eq!(classify_incident(0, 0, true, true, "verified"), "healthy");
    }
}
