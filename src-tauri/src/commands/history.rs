use crate::history::{self, HistoryEntry};
use tauri::AppHandle;

#[tauri::command]
pub fn list_history(app: AppHandle) -> Result<Vec<HistoryEntry>, String> {
    history::load(&app)
}

#[tauri::command]
pub fn clear_history(app: AppHandle) -> Result<(), String> {
    history::clear(&app)
}
