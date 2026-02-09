use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::unbounded;
use rdev::EventType;

#[derive(Clone, Debug)]
struct ClickEvent {
    timestamp_ms: u64,
    x: f64,
    y: f64,
}

#[derive(Clone, Debug)]
struct KeyEvent {
    timestamp_ms: u64,
    key: Option<String>,
    text: Option<String>,
}

#[derive(Clone, Debug)]
enum InputEvent {
    Click(ClickEvent),
    Key(KeyEvent),
}

struct RecorderState {
    is_recording: bool,
    session_id: Option<String>,
    click_events: Vec<ClickEvent>,
    key_events: Vec<KeyEvent>,
}

impl RecorderState {
    fn new() -> Self {
        Self {
            is_recording: false,
            session_id: None,
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

fn spawn_global_input_listener(state: Arc<Mutex<RecorderState>>) {
    let (sender, receiver) = unbounded::<InputEvent>();

    thread::spawn(move || {
        for event in receiver.iter() {
            let mut state = match state.lock() {
                Ok(locked) => locked,
                Err(_) => continue,
            };

            if state.is_recording {
                match event {
                    InputEvent::Click(click) => state.click_events.push(click),
                    InputEvent::Key(key_event) => state.key_events.push(key_event),
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
                    }));
                }
            }
            EventType::KeyPress(key) => {
                let _ = sender.send(InputEvent::Key(KeyEvent {
                    timestamp_ms: now_millis(),
                    key: Some(format!("{key:?}")),
                    text: None,
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
fn start_recording(state: tauri::State<Arc<Mutex<RecorderState>>>) -> Result<String, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Recording state lock poisoned".to_string())?;

    if state.is_recording {
        return Err("Recording already in progress".to_string());
    }

    let session_id = new_session_id();
    state.is_recording = true;
    state.session_id = Some(session_id.clone());
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
