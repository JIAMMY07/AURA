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
$signature=Get-AuthenticodeSignature -LiteralPath $env:AURA_SECURITY_EXE
[pscustomobject]@{
 DefenderService=[bool]$mp.AMServiceEnabled
 Realtime=[bool]$mp.RealTimeProtectionEnabled
 Antivirus=[bool]$mp.AntivirusEnabled
 SignaturesDays=[int]$mp.AntivirusSignatureAge
 QuickScanAge=[int]$mp.QuickScanAge
 FirewallDomain=[bool](($fw|Where-Object Name -eq Domain).Enabled)
 FirewallPrivate=[bool](($fw|Where-Object Name -eq Private).Enabled)
 FirewallPublic=[bool](($fw|Where-Object Name -eq Public).Enabled)
 Uac=($uac -eq 1)
 SecureBoot=$secureBoot
 WindowsUpdateDisabled=($wu.StartType -eq 'Disabled')
 BitLocker=if($bitlocker){[string]$bitlocker.ProtectionStatus}else{$null}
 SignatureStatus=[string]$signature.Status
 Signer=if($signature.SignerCertificate){[string]$signature.SignerCertificate.Subject}else{''}
} | ConvertTo-Json -Compress
"#;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawStatus {
    defender_service: bool,
    realtime: bool,
    antivirus: bool,
    signatures_days: i64,
    quick_scan_age: i64,
    firewall_domain: bool,
    firewall_private: bool,
    firewall_public: bool,
    uac: bool,
    secure_boot: Option<bool>,
    windows_update_disabled: bool,
    bit_locker: Option<String>,
    signature_status: String,
    signer: String,
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
    posture_score: u8,
    checks: Vec<SecurityCheck>,
    executable_sha256: String,
    public_signature: bool,
    local_signature: bool,
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
        status.defender_service && status.antivirus,
        20,
        "servizio antivirus e motore",
    );
    push_bool(
        &mut checks,
        &mut score,
        "realtime",
        "Protezione in tempo reale",
        status.realtime,
        15,
        "monitoraggio Defender",
    );
    push_bool(
        &mut checks,
        &mut score,
        "firewall",
        "Windows Firewall",
        status.firewall_domain && status.firewall_private && status.firewall_public,
        15,
        "profili Domain, Private e Public",
    );
    push_bool(
        &mut checks,
        &mut score,
        "uac",
        "Controllo account utente",
        status.uac,
        10,
        "EnableLUA",
    );
    let signatures_ok = status.signatures_days <= 3;
    if signatures_ok {
        score += 10;
    }
    checks.push(SecurityCheck {
        id: "definitions",
        label: "Definizioni antivirus",
        status: if signatures_ok { "healthy" } else { "warning" },
        detail: format!("età: {} giorni", status.signatures_days),
    });
    let scan_ok = status.quick_scan_age <= 7;
    if scan_ok {
        score += 10;
    }
    checks.push(SecurityCheck {
        id: "scan",
        label: "Scansione rapida",
        status: if scan_ok { "healthy" } else { "warning" },
        detail: format!("ultima: {} giorni", status.quick_scan_age),
    });
    match status.secure_boot {
        Some(true) => {
            score += 10;
            checks.push(SecurityCheck {
                id: "secure-boot",
                label: "Secure Boot",
                status: "healthy",
                detail: "attivo".into(),
            });
        }
        Some(false) => checks.push(SecurityCheck {
            id: "secure-boot",
            label: "Secure Boot",
            status: "warning",
            detail: "disattivato".into(),
        }),
        None => checks.push(SecurityCheck {
            id: "secure-boot",
            label: "Secure Boot",
            status: "unavailable",
            detail: "lettura non disponibile nella sessione corrente".into(),
        }),
    }
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
    if bitlocker_ok {
        score += 5;
    }
    checks.push(SecurityCheck {
        id: "bitlocker",
        label: "Cifratura unità",
        status: if bitlocker_ok {
            "healthy"
        } else if status.bit_locker.is_some() {
            "warning"
        } else {
            "unavailable"
        },
        detail: status
            .bit_locker
            .unwrap_or_else(|| "verifica amministrativa richiesta".into()),
    });
    let public_signature = status.signature_status.eq_ignore_ascii_case("Valid")
        && !status.signer.contains("AURA Local Development");
    let local_signature = !matches!(status.signature_status.as_str(), "NotSigned" | "");
    append_audit(&state,"security-center-status","security.inspected","allow","Postura sicurezza Windows verificata",&serde_json::json!({"score":score,"publicSignature":public_signature,"hash":hash,"sensitiveLogged":false}).to_string())?;
    Ok(SecurityCenterStatus {
        module_complete: true,
        posture_score: score,
        checks,
        executable_sha256: hash,
        public_signature,
        local_signature,
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
    use super::push_bool;
    #[test]
    fn posture_weights_are_deterministic() {
        let mut checks = Vec::new();
        let mut score = 0;
        push_bool(&mut checks, &mut score, "x", "X", true, 20, "ok");
        push_bool(&mut checks, &mut score, "y", "Y", false, 10, "no");
        assert_eq!(score, 20);
        assert_eq!(checks.len(), 2);
    }
}
