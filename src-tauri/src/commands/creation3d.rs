use super::{
    audit::append_audit, pc::confirm_native_action, policy::enforce_capability, storage::AppState,
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use tauri::State;
use wait_timeout::ChildExt;

const GENERATOR: &str = include_str!("../../../scripts/generate-3d.ps1");

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Creation3dStatus {
    installed: bool,
    triposr: bool,
    blender: bool,
    output_root: String,
    reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Creation3dResult {
    output_path: String,
    device: String,
    format: String,
}

fn roots() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let local = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "LOCALAPPDATA non disponibile.".to_string())?;
    let aura = PathBuf::from(local).join("AURA");
    Ok((
        aura.join("tools/3d/TripoSR"),
        aura.join("tools/3d/Blender-5.2.0"),
        aura.join("data/3d/jobs"),
    ))
}

#[tauri::command]
pub fn creation3d_status() -> Result<Creation3dStatus, String> {
    enforce_capability("3d.inspect", "allow")?;
    let (triposr, blender, output) = roots()?;
    let triposr_ok =
        triposr.join(".venv/Scripts/python.exe").is_file() && triposr.join("run.py").is_file();
    let blender_ok = find_blender(&blender).is_some();
    Ok(Creation3dStatus {
        installed: triposr_ok && blender_ok,
        triposr: triposr_ok,
        blender: blender_ok,
        output_root: output.to_string_lossy().to_string(),
        reason: if triposr_ok && blender_ok {
            "TripoSR e Blender locali pronti."
        } else {
            "Runtime 3D locale incompleto."
        }
        .to_string(),
    })
}

#[tauri::command]
pub async fn creation3d_generate(
    input_image: String,
    job_name: String,
    device: String,
    mesh_resolution: u16,
    format: String,
    bake_texture: bool,
    state: State<'_, AppState>,
) -> Result<Creation3dResult, String> {
    enforce_capability("3d.generate", "confirm")?;
    validate_job(&input_image, &job_name, &device, mesh_resolution, &format)?;
    if !confirm_native_action("Conferma generazione 3D", &format!("Generare localmente la mesh '{job_name}' dall'immagine selezionata? L'operazione può usare molta CPU/GPU.")) {
        return Err("Generazione 3D annullata dall'utente.".to_string());
    }
    let script = materialize_script(&state.app_data)?;
    let input = input_image.clone();
    let job = job_name.clone();
    let selected_device = device.clone();
    let selected_format = format.clone();
    let output = tauri::async_runtime::spawn_blocking(move || {
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-InputImage")
            .arg(input)
            .arg("-JobName")
            .arg(job)
            .arg("-Device")
            .arg(&selected_device)
            .arg("-MeshResolution")
            .arg(mesh_resolution.to_string())
            .arg("-Format")
            .arg(&selected_format)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if bake_texture {
            command.arg("-BakeTexture");
        }
        let mut child = command
            .spawn()
            .map_err(|_| "Impossibile avviare il runtime 3D.".to_string())?;
        match child
            .wait_timeout(Duration::from_secs(900))
            .map_err(|_| "Attesa runtime 3D fallita.".to_string())?
        {
            Some(_) => {
                let result = child
                    .wait_with_output()
                    .map_err(|_| "Lettura risultato 3D fallita.".to_string())?;
                if !result.status.success() {
                    return Err("TripoSR non ha completato la generazione.".to_string());
                }
                let stdout = String::from_utf8_lossy(&result.stdout);
                stdout
                    .lines()
                    .rev()
                    .map(str::trim)
                    .find(|line| line.ends_with(".glb") || line.ends_with(".obj"))
                    .map(str::to_string)
                    .ok_or_else(|| "Output mesh non trovato.".to_string())
            }
            None => {
                let _ = child.kill();
                Err("Generazione 3D interrotta dopo 15 minuti.".to_string())
            }
        }
    })
    .await
    .map_err(|_| "Worker 3D terminato in modo anomalo.".to_string())??;
    append_audit(
        &state,
        "creation3d-generate",
        "3d.generated",
        "confirm",
        "Mesh locale generata",
        &serde_json::json!({"job":job_name,"format":format,"device":device,"inputLogged":false})
            .to_string(),
    )?;
    Ok(Creation3dResult {
        output_path: output,
        device,
        format,
    })
}

#[tauri::command]
pub fn creation3d_open_blender(
    mesh_path: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    enforce_capability("3d.open", "confirm")?;
    let (_, blender_root, output_root) = roots()?;
    let mesh = fs::canonicalize(&mesh_path).map_err(|_| "Mesh non trovata.".to_string())?;
    let output = fs::canonicalize(output_root)
        .map_err(|_| "Cartella output 3D non disponibile.".to_string())?;
    if !mesh.starts_with(&output)
        || !matches!(
            mesh.extension().and_then(|v| v.to_str()),
            Some("glb" | "obj")
        )
    {
        return Err("AURA apre soltanto mesh prodotte nel proprio output 3D.".to_string());
    }
    if !confirm_native_action(
        "Conferma apertura Blender",
        "Aprire la mesh generata in Blender?",
    ) {
        return Ok(false);
    }
    let blender =
        find_blender(&blender_root).ok_or_else(|| "Blender locale non trovato.".to_string())?;
    Command::new(blender)
        .arg(&mesh)
        .spawn()
        .map_err(|_| "Avvio Blender fallito.".to_string())?;
    append_audit(
        &state,
        "creation3d-open",
        "3d.opened",
        "confirm",
        "Mesh aperta in Blender",
        r#"{"pathLogged":false}"#,
    )?;
    Ok(true)
}

fn validate_job(
    input: &str,
    job: &str,
    device: &str,
    resolution: u16,
    format: &str,
) -> Result<(), String> {
    let path = Path::new(input);
    if !path.is_file()
        || !matches!(
            path.extension()
                .and_then(|v| v.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("png" | "jpg" | "jpeg" | "webp")
        )
    {
        return Err("Immagine PNG/JPG/WEBP non valida.".to_string());
    }
    if job.is_empty()
        || job.len() > 64
        || !job
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        return Err("Nome job non valido.".to_string());
    }
    if !matches!(device, "Auto" | "Gpu" | "Cpu")
        || !(64..=256).contains(&resolution)
        || !matches!(format, "glb" | "obj")
    {
        return Err("Profilo 3D fuori dai limiti.".to_string());
    }
    Ok(())
}

fn materialize_script(app_data: &Path) -> Result<PathBuf, String> {
    let root = app_data.join("runtime");
    fs::create_dir_all(&root).map_err(|_| "Cartella runtime non disponibile.".to_string())?;
    let path = root.join("generate-3d.ps1");
    fs::write(&path, GENERATOR).map_err(|_| "Installazione launcher 3D fallita.".to_string())?;
    Ok(path)
}

fn find_blender(root: &Path) -> Option<PathBuf> {
    let direct = root.join("blender.exe");
    if direct.is_file() {
        return Some(direct);
    }
    fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path().join("blender.exe"))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::validate_job;
    #[test]
    fn rejects_untyped_3d_inputs() {
        assert!(validate_job("missing.exe", "safe", "Auto", 128, "glb").is_err());
        assert!(validate_job("missing.png", "../escape", "Auto", 128, "glb").is_err());
    }
}
