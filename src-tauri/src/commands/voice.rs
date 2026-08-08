use super::{audit::append_audit, policy::enforce_capability, storage::AppState};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::State;
use uuid::Uuid;
use wait_timeout::ChildExt;

const MAX_WAV_BYTES: usize = 2 * 1024 * 1024;
const MAX_BASE64_BYTES: usize = MAX_WAV_BYTES.div_ceil(3) * 4;
const MAX_AUDIO_SECONDS: f64 = 30.5;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    available: bool,
    engine: &'static str,
    model: &'static str,
    vad: bool,
    max_seconds: u32,
    tts_available: bool,
    tts_engine: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTranscript {
    text: String,
    duration_seconds: f64,
    confidence: f64,
}

#[tauri::command]
pub fn voice_status(state: State<'_, AppState>) -> VoiceStatus {
    let paths = voice_paths(&state.app_data);
    VoiceStatus {
        available: paths.cli.is_file() && paths.model.is_file() && paths.vad_model.is_file(),
        engine: "whisper.cpp",
        model: "base multilingual",
        vad: paths.vad_model.is_file(),
        max_seconds: 30,
        tts_available: cfg!(windows),
        tts_engine: "Windows SAPI",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSpeakResult {
    started: bool,
    characters: usize,
}

#[tauri::command]
pub fn voice_speak(text: String, state: State<'_, AppState>) -> Result<VoiceSpeakResult, String> {
    enforce_capability("voice.speak", "allow")?;
    let text = validate_tts_text(&text)?;
    let mut active = state
        .tts_process
        .lock()
        .map_err(|_| "Motore TTS occupato.".to_string())?;
    if let Some(mut previous) = active.take() {
        let _ = previous.kill();
        let _ = previous.wait();
    }
    let script = "$ErrorActionPreference='Stop'; $text=[Console]::In.ReadToEnd(); Add-Type -AssemblyName System.Speech; $speaker=New-Object System.Speech.Synthesis.SpeechSynthesizer; $it=$speaker.GetInstalledVoices() | Where-Object { $_.VoiceInfo.Culture.Name -like 'it-*' } | Select-Object -First 1; if($it){$speaker.SelectVoice($it.VoiceInfo.Name)}; $speaker.Rate=0; $speaker.Volume=90; $speaker.Speak($text)";
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Avvio sintesi vocale Windows fallito.".to_string())?;
    child
        .stdin
        .take()
        .ok_or_else(|| "Canale TTS non disponibile.".to_string())?
        .write_all(text.as_bytes())
        .map_err(|_| "Invio testo al motore TTS fallito.".to_string())?;
    let characters = text.chars().count();
    *active = Some(child);
    append_audit(
        &state,
        &Uuid::new_v4().to_string(),
        "voice.tts.started",
        "allow",
        "Sintesi vocale locale avviata",
        &serde_json::json!({"characters": characters, "contentLogged": false}).to_string(),
    )?;
    Ok(VoiceSpeakResult {
        started: true,
        characters,
    })
}

fn validate_tts_text(text: &str) -> Result<&str, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 4_000 || text.contains('\0') {
        Err("Testo TTS fuori dai limiti (1-4000 caratteri).".to_string())
    } else {
        Ok(text)
    }
}

#[tauri::command]
pub fn voice_stop_speaking(state: State<'_, AppState>) -> Result<bool, String> {
    enforce_capability("voice.speak", "allow")?;
    let mut active = state
        .tts_process
        .lock()
        .map_err(|_| "Motore TTS occupato.".to_string())?;
    let Some(mut child) = active.take() else {
        return Ok(false);
    };
    let _ = child.kill();
    let _ = child.wait();
    append_audit(
        &state,
        &Uuid::new_v4().to_string(),
        "voice.tts.stopped",
        "allow",
        "Sintesi vocale locale arrestata",
        r#"{"contentLogged":false}"#,
    )?;
    Ok(true)
}

#[tauri::command]
pub async fn voice_transcribe(
    wav_base64: String,
    state: State<'_, AppState>,
) -> Result<VoiceTranscript, String> {
    enforce_capability("voice.transcribe", "allow")?;
    if wav_base64.len() > MAX_BASE64_BYTES {
        return Err("Registrazione vocale troppo grande.".to_string());
    }
    let busy = VoiceBusyGuard::acquire(Arc::clone(&state.voice_busy))?;
    let app_data = state.app_data.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _busy = busy;
        transcribe(&app_data, &wav_base64)
    })
    .await
    .map_err(|_| "Worker vocale interrotto.".to_string())?
}

struct VoiceBusyGuard(Arc<AtomicBool>);

impl VoiceBusyGuard {
    fn acquire(flag: Arc<AtomicBool>) -> Result<Self, String> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self(flag))
            .map_err(|_| "Una trascrizione vocale è già in corso.".to_string())
    }
}

impl Drop for VoiceBusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct VoicePaths {
    cli: PathBuf,
    model: PathBuf,
    vad_model: PathBuf,
    temporary: PathBuf,
}

fn voice_paths(app_data: &Path) -> VoicePaths {
    let root = app_data.join("voice");
    VoicePaths {
        cli: root.join("bin").join("whisper-cli.exe"),
        model: root.join("models").join("ggml-base.bin"),
        vad_model: root.join("models").join("ggml-silero-v6.2.0.bin"),
        temporary: root.join("tmp"),
    }
}

pub(crate) fn cleanup_stale_audio(app_data: &Path) -> std::io::Result<()> {
    let temporary = app_data.join("voice").join("tmp");
    if !temporary.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&temporary)? {
        let entry = entry?;
        let path = entry.path();
        let disposable = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| matches!(extension, "wav" | "json"));
        if disposable && entry.file_type()?.is_file() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn transcribe(app_data: &Path, encoded: &str) -> Result<VoiceTranscript, String> {
    let paths = voice_paths(app_data);
    if !paths.cli.is_file() || !paths.model.is_file() || !paths.vad_model.is_file() {
        return Err(
            "Motore vocale locale non installato. Esegui scripts/setup-whisper.ps1.".to_string(),
        );
    }
    let wav = STANDARD
        .decode(encoded)
        .map_err(|_| "Registrazione vocale non valida.".to_string())?;
    if wav.len() > MAX_WAV_BYTES {
        return Err("Registrazione vocale troppo grande.".to_string());
    }
    let duration_seconds = validate_wav(&wav)?;
    fs::create_dir_all(&paths.temporary)
        .map_err(|_| "Cartella temporanea vocale non disponibile.".to_string())?;
    let id = Uuid::new_v4().to_string();
    let wav_path = paths.temporary.join(format!("{id}.wav"));
    let output_prefix = paths.temporary.join(&id);
    let json_path = paths.temporary.join(format!("{id}.json"));
    fs::write(&wav_path, &wav).map_err(|_| "Scrittura audio temporaneo fallita.".to_string())?;

    let result = run_whisper(&paths, &wav_path, &output_prefix)
        .and_then(|_| parse_transcript(&json_path, duration_seconds));
    let _ = fs::remove_file(&wav_path);
    let _ = fs::remove_file(&json_path);
    result.and_then(|transcript| {
        if transcript.text.is_empty() {
            Err("Nessun parlato riconosciuto.".to_string())
        } else {
            Ok(transcript)
        }
    })
}

fn run_whisper(paths: &VoicePaths, wav_path: &Path, output_prefix: &Path) -> Result<(), String> {
    let mut command = Command::new(&paths.cli);
    command
        .arg("-m")
        .arg(&paths.model)
        .arg("-f")
        .arg(wav_path)
        .arg("-l")
        .arg("it")
        .arg("-vm")
        .arg(&paths.vad_model)
        .arg("--vad")
        .arg("-nt")
        .arg("-np")
        .arg("-ojf")
        .arg("-of")
        .arg(output_prefix)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Avvio motore vocale locale fallito.".to_string())?;
    match child
        .wait_timeout(Duration::from_secs(90))
        .map_err(|_| "Attesa motore vocale fallita.".to_string())?
    {
        Some(status) if status.success() => Ok(()),
        Some(_) => Err("Trascrizione locale fallita.".to_string()),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Err("Trascrizione locale oltre il limite di 90 secondi.".to_string())
        }
    }
}

fn parse_transcript(path: &Path, duration_seconds: f64) -> Result<VoiceTranscript, String> {
    let raw =
        fs::read_to_string(path).map_err(|_| "Trascrizione locale non prodotta.".to_string())?;
    let payload: serde_json::Value =
        serde_json::from_str(&raw).map_err(|_| "Risultato Whisper non valido.".to_string())?;
    let segments = payload
        .get("transcription")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Segmenti Whisper mancanti.".to_string())?;
    let text = segments
        .iter()
        .filter_map(|segment| segment.get("text").and_then(serde_json::Value::as_str))
        .collect::<String>()
        .trim()
        .to_string();
    let probabilities = segments
        .iter()
        .filter_map(|segment| segment.get("tokens").and_then(serde_json::Value::as_array))
        .flatten()
        .filter(|token| {
            !token
                .get("text")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| text.starts_with("[_"))
        })
        .filter_map(|token| token.get("p").and_then(serde_json::Value::as_f64))
        .collect::<Vec<_>>();
    let confidence = if probabilities.is_empty() {
        0.0
    } else {
        probabilities.iter().sum::<f64>() / probabilities.len() as f64
    };
    Ok(VoiceTranscript {
        text,
        duration_seconds,
        confidence: confidence.clamp(0.0, 1.0),
    })
}

fn validate_wav(bytes: &[u8]) -> Result<f64, String> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Formato audio non WAV.".to_string());
    }
    let riff_size = u32::from_le_bytes(
        bytes[4..8]
            .try_into()
            .map_err(|_| "Header WAV non valido.".to_string())?,
    ) as usize;
    let logical_end = riff_size
        .checked_add(8)
        .filter(|end| *end >= 44 && *end <= bytes.len())
        .ok_or_else(|| "Dimensione RIFF non valida.".to_string())?;
    let mut offset = 12_usize;
    let mut valid_format = false;
    let mut data_bytes = None;
    while offset.saturating_add(8) <= logical_end {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        let end = start.saturating_add(size);
        if end > logical_end {
            return Err("Struttura WAV non valida.".to_string());
        }
        if id == b"fmt " && size >= 16 {
            let format = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap());
            let channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap());
            let sample_rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
            let byte_rate = u32::from_le_bytes(bytes[start + 8..start + 12].try_into().unwrap());
            let block_align = u16::from_le_bytes(bytes[start + 12..start + 14].try_into().unwrap());
            let bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap());
            valid_format = format == 1
                && channels == 1
                && sample_rate == 16_000
                && byte_rate == 32_000
                && block_align == 2
                && bits == 16;
        } else if id == b"data" {
            if !size.is_multiple_of(2) {
                return Err("Chunk PCM16 con dimensione non valida.".to_string());
            }
            data_bytes = Some(size);
        }
        offset = end + (size % 2);
    }
    if !valid_format {
        return Err("Audio richiesto: PCM mono 16 kHz a 16 bit.".to_string());
    }
    let data_bytes = data_bytes.ok_or_else(|| "Chunk audio WAV mancante.".to_string())?;
    let duration = data_bytes as f64 / 32_000.0;
    if !(0.15..=MAX_AUDIO_SECONDS).contains(&duration) {
        return Err("Durata registrazione fuori dai limiti (0,15–30 secondi).".to_string());
    }
    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::{
        cleanup_stale_audio, parse_transcript, validate_tts_text, validate_wav, VoiceBusyGuard,
    };
    use std::sync::{atomic::AtomicBool, Arc};
    use std::{fs, io::Write};

    fn wav(seconds: f64) -> Vec<u8> {
        let samples = (16_000.0 * seconds) as u32;
        let data_size = samples * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&32_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.resize(44 + data_size as usize, 0);
        bytes
    }

    #[test]
    fn removes_only_stale_voice_artifacts_on_startup() {
        let root = std::env::temp_dir().join(format!("aura-voice-test-{}", uuid::Uuid::new_v4()));
        let temporary = root.join("voice").join("tmp");
        fs::create_dir_all(&temporary).expect("temporary folder");
        fs::write(temporary.join("stale.wav"), b"synthetic audio").expect("wav fixture");
        fs::write(temporary.join("stale.json"), b"{}").expect("json fixture");
        fs::write(temporary.join("keep.txt"), b"keep").expect("unrelated fixture");
        cleanup_stale_audio(&root).expect("cleanup");
        assert!(!temporary.join("stale.wav").exists());
        assert!(!temporary.join("stale.json").exists());
        assert!(temporary.join("keep.txt").exists());
        fs::remove_dir_all(&root).expect("remove test fixture");
    }

    #[test]
    fn accepts_bounded_pcm_wav() {
        let duration = validate_wav(&wav(1.0)).expect("valid wav");
        assert!((duration - 1.0).abs() < 0.01);
    }

    #[test]
    fn rejects_wrong_format_and_long_audio() {
        assert!(validate_wav(b"not a wav").is_err());
        assert!(validate_wav(&wav(31.0)).is_err());
        let mut invalid_riff = wav(1.0);
        invalid_riff[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_wav(&invalid_riff).is_err());
    }

    #[test]
    fn extracts_text_and_average_token_confidence() {
        let mut file = tempfile::NamedTempFile::new().expect("temporary json");
        write!(
            file,
            r#"{{"transcription":[{{"text":" ciao","tokens":[{{"text":" ci","p":0.8}},{{"text":"ao","p":0.6}},{{"text":"[_EOT_]","p":1.0}}]}}]}}"#
        )
        .expect("write json");
        let result = parse_transcript(file.path(), 1.0).expect("parse transcript");
        assert_eq!(result.text, "ciao");
        assert!((result.confidence - 0.7).abs() < 0.001);
    }

    #[test]
    fn allows_only_one_voice_worker_at_a_time() {
        let flag = Arc::new(AtomicBool::new(false));
        let first = VoiceBusyGuard::acquire(Arc::clone(&flag)).expect("first worker");
        assert!(VoiceBusyGuard::acquire(Arc::clone(&flag)).is_err());
        drop(first);
        assert!(VoiceBusyGuard::acquire(flag).is_ok());
    }

    #[test]
    fn bounds_tts_text_without_interpreting_content() {
        assert_eq!(validate_tts_text(" ciao ").expect("text"), "ciao");
        assert!(validate_tts_text("").is_err());
        assert!(validate_tts_text(&"a".repeat(4_001)).is_err());
        assert!(validate_tts_text("ciao\0mondo").is_err());
        assert!(validate_tts_text("$(Remove-Item *)").is_ok());
    }
}
