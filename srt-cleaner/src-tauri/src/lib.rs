use tauri_plugin_dialog::DialogExt;

#[tauri::command]
async fn save_srt(
    app: tauri::AppHandle,
    filename: String,
    content: String,
) -> Result<String, String> {
    let path = app
        .dialog()
        .file()
        .set_file_name(&filename)
        .add_filter("SRT subtitle", &["srt"])
        .blocking_save_file();

    match path {
        Some(file_path) => {
            let path_buf = file_path.as_path().ok_or("Invalid path")?.to_path_buf();
            std::fs::write(&path_buf, content.as_bytes()).map_err(|e| e.to_string())?;
            Ok(path_buf.to_string_lossy().into_owned())
        }
        None => Err("cancelled".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![save_srt])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
