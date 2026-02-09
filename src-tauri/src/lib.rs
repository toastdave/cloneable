use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::unbounded;
use rdev::EventType;
use screenshots::Screen;
use tauri::Manager;

#[derive(Clone, Debug)]
struct ClickEvent {
    timestamp_ms: u64,
    x: f64,
    y: f64,
    full_screenshot_path: Option<String>,
    full_screenshot_error: Option<String>,
}

#[derive(Clone, Debug)]
struct KeyEvent {
    timestamp_ms: u64,
    key: Option<String>,
    text: Option<String>,
    full_screenshot_path: Option<String>,
    full_screenshot_error: Option<String>,
}

#[derive(Clone, Debug)]
enum InputEvent {
    Click(ClickEvent),
    Key(KeyEvent),
}

struct RecorderState {
    is_recording: bool,
    session_id: Option<String>,
    shots_dir: Option<PathBuf>,
    click_events: Vec<ClickEvent>,
    key_events: Vec<KeyEvent>,
}

impl RecorderState {
    fn new() -> Self {
        Self {
            is_recording: false,
            session_id: None,
            shots_dir: None,
            click_events: Vec::new(),
            key_events: Vec::new(),
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

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn capture_full_screenshot(
    shots_dir: &PathBuf,
    prefix: &str,
    timestamp_ms: u64,
) -> Result<String, String> {
    let screens = Screen::all().map_err(|error| format!("Screen list failed: {error}"))?;
    let screen = screens
        .first()
        .ok_or_else(|| "No screen available for capture".to_string())?;
    let image = screen
        .capture()
        .map_err(|error| format!("Screen capture failed: {error}"))?;
    let filename = format!("{prefix}-{timestamp_ms}.png");
    let path = shots_dir.join(filename);
    image
        .save(&path)
        .map_err(|error| format!("Screenshot save failed: {error}"))?;
    Ok(path.to_string_lossy().to_string())
}

fn spawn_global_input_listener(state: Arc<Mutex<RecorderState>>) {
    let (sender, receiver) = unbounded::<InputEvent>();

    thread::spawn(move || {
        for event in receiver.iter() {
            let recorder_state = match state.lock() {
                Ok(locked) => locked,
                Err(_) => continue,
            };

            if !recorder_state.is_recording {
                continue;
            }

            let session_id = recorder_state.session_id.clone();
            let shots_dir = recorder_state.shots_dir.clone();
            drop(recorder_state);

            match event {
                InputEvent::Click(mut click) => {
                    let capture_result = shots_dir
                        .as_ref()
                        .ok_or_else(|| "Screenshot directory not initialized".to_string())
                        .and_then(|dir| capture_full_screenshot(dir, "click", click.timestamp_ms));
                    match capture_result {
                        Ok(path) => click.full_screenshot_path = Some(path),
                        Err(error) => click.full_screenshot_error = Some(error),
                    }

                    let mut recorder_state = match state.lock() {
                        Ok(locked) => locked,
                        Err(_) => continue,
                    };
                    if recorder_state.session_id == session_id {
                        recorder_state.click_events.push(click);
                    }
                }
                InputEvent::Key(mut key_event) => {
                    let capture_result = shots_dir
                        .as_ref()
                        .ok_or_else(|| "Screenshot directory not initialized".to_string())
                        .and_then(|dir| {
                            capture_full_screenshot(dir, "key", key_event.timestamp_ms)
                        });
                    match capture_result {
                        Ok(path) => key_event.full_screenshot_path = Some(path),
                        Err(error) => key_event.full_screenshot_error = Some(error),
                    }

                    let mut recorder_state = match state.lock() {
                        Ok(locked) => locked,
                        Err(_) => continue,
                    };
                    if recorder_state.session_id == session_id {
                        recorder_state.key_events.push(key_event);
                    }
                }
            }
        }
    });

    thread::spawn(move || {
        let mut last_position: Option<(f64, f64)> = None;
        let result = rdev::listen(move |event| match event.event_type {
            EventType::MouseMove { x, y } => {
                last_position = Some((x, y));
            }
            EventType::ButtonPress(_) => {
                if let Some((x, y)) = last_position {
                    let _ = sender.send(InputEvent::Click(ClickEvent {
                        timestamp_ms: now_millis(),
                        x,
                        y,
                        full_screenshot_path: None,
                        full_screenshot_error: None,
                    }));
                }
            }
            EventType::KeyPress(key) => {
                let _ = sender.send(InputEvent::Key(KeyEvent {
                    timestamp_ms: now_millis(),
                    key: Some(format!("{key:?}")),
                    text: None,
                    full_screenshot_path: None,
                    full_screenshot_error: None,
                }));
            }
            _ => {}
        });

        if let Err(error) = result {
            eprintln!("Global input listener stopped: {error:?}");
        }
    });
}

#[tauri::command]
fn start_recording(
    app_handle: tauri::AppHandle,
    state: tauri::State<Arc<Mutex<RecorderState>>>,
) -> Result<String, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Recording state lock poisoned".to_string())?;

    if state.is_recording {
        return Err("Recording already in progress".to_string());
    }

    let session_id = new_session_id();
    let base_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let shots_dir = base_dir.join("recordings").join(&session_id).join("shots");
    if let Err(error) = fs::create_dir_all(&shots_dir) {
        eprintln!("Failed to create recordings dir: {error:?}");
    }
    state.is_recording = true;
    state.session_id = Some(session_id.clone());
    state.shots_dir = Some(shots_dir);
    state.click_events.clear();
    state.key_events.clear();

    Ok(session_id)
}

#[tauri::command]
fn stop_recording(state: tauri::State<Arc<Mutex<RecorderState>>>) -> Result<String, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Recording state lock poisoned".to_string())?;

    if !state.is_recording {
        return Err("Recording is not active".to_string());
    }

    state.is_recording = false;
    state.shots_dir = None;
    Ok(state.session_id.take().unwrap_or_else(new_session_id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let recorder_state = Arc::new(Mutex::new(RecorderState::new()));
    spawn_global_input_listener(recorder_state.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(recorder_state)
        .invoke_handler(tauri::generate_handler![start_recording, stop_recording])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
