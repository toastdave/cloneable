mod error;
mod events;
mod recorder;
mod screenshots;
mod storage;

use tauri::{Manager, State};

use crate::error::AppError;
use crate::recorder::RecorderState;

#[tauri::command]
fn start_recording(
    app: tauri::AppHandle,
    state: State<'_, RecorderState>,
) -> Result<String, AppError> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    recorder::start(&state, &base_dir)
}

#[tauri::command]
fn stop_recording(state: State<'_, RecorderState>) -> Result<String, AppError> {
    recorder::stop(&state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(RecorderState::new())
        .invoke_handler(tauri::generate_handler![start_recording, stop_recording])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
