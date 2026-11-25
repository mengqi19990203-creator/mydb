use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tauri::{Manager, State};

use super::AppState;

fn encode(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}

/// Whether a standalone query window currently uses the connection. Scope
/// switches are blocked in that case so every window keeps the same active
/// database / schema.
#[tauri::command]
pub fn connection_has_query_window(state: State<'_, AppState>, connection_id: String) -> bool {
    super::query_window_uses_connection(&state, &connection_id)
}

/// Opens a standalone query window. The new webview loads the same frontend
/// with `window=query` and receives its initial connection / database / SQL
/// through URL parameters, so it does not share tabs or editor state with the
/// main window.
#[tauri::command]
pub fn open_query_window(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    title: Option<String>,
    connection_id: Option<String>,
    database: Option<String>,
    schema: Option<String>,
    sql: Option<String>,
) -> Result<String, String> {
    let label = format!("query-{}", uuid::Uuid::new_v4().simple());
    let connection_id = connection_id.filter(|value| !value.is_empty());
    let mut url = String::from("index.html?window=query");
    if let Some(value) = title.as_deref().filter(|value| !value.trim().is_empty()) {
        url.push_str("&title=");
        url.push_str(&encode(value));
    }
    if let Some(value) = connection_id.as_deref() {
        url.push_str("&connectionId=");
        url.push_str(&encode(value));
    }
    if let Some(value) = database.as_deref().filter(|value| !value.is_empty()) {
        url.push_str("&database=");
        url.push_str(&encode(value));
    }
    if let Some(value) = schema.as_deref().filter(|value| !value.is_empty()) {
        url.push_str("&schema=");
        url.push_str(&encode(value));
    }
    if let Some(value) = sql.as_deref().filter(|value| !value.is_empty()) {
        url.push_str("&sql=");
        url.push_str(&encode(value));
    }
    let window_title = title
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("mydb · {value}"))
        .unwrap_or_else(|| "mydb · 查询".to_string());

    if let Some(connection_id) = connection_id.as_deref() {
        state
            .query_windows
            .lock()
            .map_err(|_| "查询窗口状态不可用".to_string())?
            .insert(label.clone(), connection_id.to_string());
    }

    let window =
        match tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App(url.into()))
            .title(window_title)
            .inner_size(1080.0, 760.0)
            .min_inner_size(720.0, 480.0)
            .center()
            .build()
        {
            Ok(window) => window,
            Err(error) => {
                if let Ok(mut windows) = state.query_windows.lock() {
                    windows.remove(&label);
                }
                return Err(format!("无法打开查询窗口：{error}"));
            }
        };

    let app_handle = app.clone();
    let cleanup_label = label.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let state = app_handle.state::<AppState>();
            if let Ok(mut windows) = state.query_windows.lock() {
                windows.remove(&cleanup_label);
            }
        }
    });
    Ok(label)
}
