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
import os
os.environ.pop('HUGGINGFACE_HUB_TOKEN', None)
os.environ.pop('HF_TOKEN', None)
os.environ['HF_HUB_DISABLE_IMPLICIT_TOKEN'] = '1'
from huggingface_hub import snapshot_download
snapshot_download(repo_id="mlx-community/whisper-large-v3-mlx", token=False)
print("done")
"#;

    let script_path = "/tmp/download_model.py";
    std::fs::write(script_path, script).map_err(|e| e.to_string())?;

    let output = Command::new("python3")
        .arg(script_path)
        .env("PATH", "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin")
        .env("HF_HUB_DISABLE_IMPLICIT_TOKEN", "1")
        .env_remove("HUGGINGFACE_HUB_TOKEN")
        .env_remove("HF_TOKEN")
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
import mlx_whisper, sys, io, subprocess

video_path = r"""{video_path}"""
srt_path = r"""{srt_path}"""
progress_file = "/tmp/transcribe_progress.txt"

try:
    probe = subprocess.run(
        ['ffprobe', '-v', 'error', '-show_entries', 'format=duration',
         '-of', 'default=noprint_wrappers=1:nokey=1', video_path],
        capture_output=True, text=True,
        env={{'PATH': '/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin'}}
    )
    total_duration = float(probe.stdout.strip() or '0') or 1.0
except:
    total_duration = 1.0

open(progress_file, 'w').write('0')
real_stdout = sys.stdout

class ProgressOut(io.TextIOBase):
    def writable(self): return True
    def write(self, s):
        arrow = s.find(' --> ')
        if arrow >= 0:
            end_part = s[arrow + 5:]
            end_ts = end_part.split(']')[0] if ']' in end_part else ''
            parts = end_ts.split(':')
            try:
                if len(parts) == 2:
                    secs = float(parts[0]) * 60 + float(parts[1])
                elif len(parts) == 3:
                    secs = float(parts[0]) * 3600 + float(parts[1]) * 60 + float(parts[2])
                else:
                    return len(s)
                pct = min(int(secs / total_duration * 100), 99)
                open(progress_file, 'w').write(str(pct))
            except:
                pass
        return len(s)
    def flush(self): pass

sys.stdout = ProgressOut()

result = mlx_whisper.transcribe(
    video_path,
    path_or_hf_repo="mlx-community/whisper-large-v3-mlx",
    word_timestamps=True,
    verbose=True
)

sys.stdout = real_stdout

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

with open(srt_path, "w", encoding="utf-8") as f:
    f.write("\n".join(lines))

open(progress_file, 'w').write('100')
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

#[tauri::command]
async fn transcribe_text(video_path: String) -> Result<String, String> {
    let script = format!(
        r#"
import mlx_whisper, sys, io, subprocess

video_path = r"""{video_path}"""
progress_file = "/tmp/transcribe_progress.txt"
result_file = "/tmp/transcribe_text_result.txt"

try:
    probe = subprocess.run(
        ['ffprobe', '-v', 'error', '-show_entries', 'format=duration',
         '-of', 'default=noprint_wrappers=1:nokey=1', video_path],
        capture_output=True, text=True,
        env={{'PATH': '/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin'}}
    )
    total_duration = float(probe.stdout.strip() or '0') or 1.0
except:
    total_duration = 1.0

open(progress_file, 'w').write('0')
real_stdout = sys.stdout

class ProgressOut(io.TextIOBase):
    def writable(self): return True
    def write(self, s):
        arrow = s.find(' --> ')
        if arrow >= 0:
            end_part = s[arrow + 5:]
            end_ts = end_part.split(']')[0] if ']' in end_part else ''
            parts = end_ts.split(':')
            try:
                if len(parts) == 2:
                    secs = float(parts[0]) * 60 + float(parts[1])
                elif len(parts) == 3:
                    secs = float(parts[0]) * 3600 + float(parts[1]) * 60 + float(parts[2])
                else:
                    return len(s)
                pct = min(int(secs / total_duration * 100), 99)
                open(progress_file, 'w').write(str(pct))
            except:
                pass
        return len(s)
    def flush(self): pass

sys.stdout = ProgressOut()

result = mlx_whisper.transcribe(
    video_path,
    path_or_hf_repo="mlx-community/whisper-large-v3-mlx",
    word_timestamps=False,
    verbose=True
)

sys.stdout = real_stdout

text = " ".join(seg["text"].strip() for seg in result["segments"])
with open(result_file, "w", encoding="utf-8") as f:
    f.write(text)

open(progress_file, 'w').write('100')
"#,
        video_path = video_path,
    );

    let script_path = "/tmp/mlx_transcribe_text.py";
    std::fs::write(script_path, &script).map_err(|e| e.to_string())?;

    let output = Command::new("python3")
        .arg(script_path)
        .env("PATH", "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin")
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let text = std::fs::read_to_string("/tmp/transcribe_text_result.txt")
            .map_err(|e| e.to_string())?;
        Ok(text)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(stderr)
    }
}

#[tauri::command]
fn get_progress() -> f64 {
    std::fs::read_to_string("/tmp/transcribe_progress.txt")
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
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
            transcribe_text,
            get_progress,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
