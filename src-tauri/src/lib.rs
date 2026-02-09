use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

struct RecorderState {
    is_recording: bool,
    session_id: Option<String>,
}

impl RecorderState {
    fn new() -> Self {
        Self {
            is_recording: false,
            session_id: None,
        }
    }
}

fn new_session_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("session-{}", millis)
}

#[tauri::command]
fn start_recording(state: tauri::State<Mutex<RecorderState>>) -> Result<String, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Recording state lock poisoned".to_string())?;

    if state.is_recording {
        return Err("Recording already in progress".to_string());
    }

    let session_id = new_session_id();
    state.is_recording = true;
    state.session_id = Some(session_id.clone());

    Ok(session_id)
}

#[tauri::command]
fn stop_recording(state: tauri::State<Mutex<RecorderState>>) -> Result<String, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Recording state lock poisoned".to_string())?;

    if !state.is_recording {
        return Err("Recording is not active".to_string());
    }

    state.is_recording = false;
    Ok(state.session_id.take().unwrap_or_else(new_session_id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(RecorderState::new()))
        .invoke_handler(tauri::generate_handler![start_recording, stop_recording])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
