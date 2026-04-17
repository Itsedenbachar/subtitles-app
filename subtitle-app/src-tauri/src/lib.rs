use std::path::Path;
use std::process::Command;
use tauri_plugin_updater::UpdaterExt;

#[tauri::command]
fn check_setup() -> Result<bool, String> {
    let status = Command::new("/bin/sh")
        .args(["-c", "python3 -c 'import mlx_whisper'"])
        .status()
        .map_err(|e| e.to_string())?;

    Ok(status.success())
}

#[tauri::command]
fn install_mlx_whisper() -> Result<String, String> {
    let output = Command::new("/bin/sh")
        .args(["-c", "python3 -m pip install mlx-whisper --upgrade"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok("done".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(stderr)
    }
}

#[tauri::command]
fn check_ffmpeg() -> Result<bool, String> {
    let output = Command::new("/bin/sh")
        .args(["-c", "export PATH=/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:$PATH && which ffmpeg"])
        .output()
        .map_err(|e| e.to_string())?;

    Ok(output.status.success())
}

#[tauri::command]
fn install_ffmpeg() -> Result<String, String> {
    let brew_check = Command::new("/bin/sh")
        .args(["-c", "export PATH=/usr/local/bin:/opt/homebrew/bin:$PATH && which brew"])
        .output()
        .map_err(|e| e.to_string())?;

    if !brew_check.status.success() {
        return Err("Homebrew is not installed. Please install it from brew.sh, then reopen this app.".to_string());
    }

    let output = Command::new("/bin/sh")
        .args(["-c", "export PATH=/usr/local/bin:/opt/homebrew/bin:$PATH && brew install ffmpeg"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok("done".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(stderr)
    }
}

#[tauri::command]
fn check_model_downloaded() -> Result<bool, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let model_path = format!(
        "{}/.cache/huggingface/hub/models--mlx-community--whisper-large-v3-mlx",
        home
    );

    Ok(Path::new(&model_path).exists())
}

#[tauri::command]
fn download_model() -> Result<String, String> {
    let script = r#"
from huggingface_hub import snapshot_download
snapshot_download(repo_id="mlx-community/whisper-large-v3-mlx")
print("done")
"#;

    let script_path = "/tmp/download_model.py";
    std::fs::write(script_path, script).map_err(|e| e.to_string())?;

    let output = Command::new("python3")
        .arg(script_path)
        .env("PATH", "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin")
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok("done".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(stderr)
    }
}

#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    if let Some(update) = update {
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| e.to_string())?;
        app.restart();
    }
    Ok(())
}

#[tauri::command]
async fn transcribe(video_path: String) -> Result<String, String> {
    let path = Path::new(&video_path);

    let output_dir = path
        .parent()
        .ok_or("Could not determine output directory")?
        .to_string_lossy()
        .into_owned();

    let stem = path
        .file_stem()
        .ok_or("Could not determine file stem")?
        .to_string_lossy()
        .into_owned();

    let srt_path = format!("{}/{}.srt", output_dir, stem);

    let script = format!(
        r#"
import mlx_whisper, os

result = mlx_whisper.transcribe(
    r"""{video_path}""",
    path_or_hf_repo="mlx-community/whisper-large-v3-mlx",
    language="pl",
    word_timestamps=True
)

def fmt(s):
    h = int(s // 3600)
    m = int(s % 3600 // 60)
    sec = int(s % 60)
    ms = int(round(s % 1 * 1000))
    return f"{{h:02d}}:{{m:02d}}:{{sec:02d}},{{ms:03d}}"

lines = []
for i, seg in enumerate(result["segments"], 1):
    lines.append(str(i))
    lines.append(f"{{fmt(seg['start'])}} --> {{fmt(seg['end'])}}")
    lines.append(seg["text"].strip())
    lines.append("")

with open(r"""{srt_path}""", "w", encoding="utf-8") as f:
    f.write("\n".join(lines))
"#,
        video_path = video_path,
        srt_path = srt_path,
    );

    let script_path = "/tmp/mlx_transcribe.py";
    std::fs::write(script_path, &script).map_err(|e| e.to_string())?;

    let output = Command::new("python3")
        .arg(script_path)
        .env("PATH", "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin")
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(srt_path)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(stderr)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            check_for_updates,
            check_setup,
            install_mlx_whisper,
            check_ffmpeg,
            install_ffmpeg,
            check_model_downloaded,
            download_model,
            transcribe,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
