use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn load_conversations(app: AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let file = dir.join("conversations.json");
    let backup = dir.join("conversations.bak.json");

    if let Ok(content) = std::fs::read_to_string(&file) {
        if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
            return Ok(content);
        }
    }
    if let Ok(content) = std::fs::read_to_string(&backup) {
        if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
            return Ok(content);
        }
    }
    Ok("[]".to_string())
}

#[tauri::command]
pub fn save_conversations(app: AppHandle, data: String) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&data)
        .map_err(|e| format!("Dados de conversa inválidos: {e}"))?;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let file = dir.join("conversations.json");
    let backup = dir.join("conversations.bak.json");
    let tmp = dir.join("conversations.json.tmp");

    if file.is_file() {
        let _ = std::fs::copy(&file, &backup);
    }
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &file).map_err(|e| e.to_string())
}
