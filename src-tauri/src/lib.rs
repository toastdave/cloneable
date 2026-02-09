use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::unbounded;
use image::imageops::crop_imm;
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
    window_crop_path: Option<String>,
    window_crop_error: Option<String>,
    window_crop_fallback: bool,
    click_crop_path: Option<String>,
    click_crop_error: Option<String>,
}

#[derive(Clone, Debug)]
struct KeyEvent {
    timestamp_ms: u64,
    key: Option<String>,
    text: Option<String>,
    full_screenshot_path: Option<String>,
    full_screenshot_error: Option<String>,
    window_crop_path: Option<String>,
    window_crop_error: Option<String>,
    window_crop_fallback: bool,
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

const CLICK_CROP_WIDTH: u32 = 400;
const CLICK_CROP_HEIGHT: u32 = 300;

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

fn clamp_crop_bounds(
    center_x: f64,
    center_y: f64,
    screen_width: u32,
    screen_height: u32,
) -> (u32, u32, u32, u32) {
    let crop_width = CLICK_CROP_WIDTH.min(screen_width);
    let crop_height = CLICK_CROP_HEIGHT.min(screen_height);
    let half_width = (crop_width / 2) as i64;
    let half_height = (crop_height / 2) as i64;
    let mut left = center_x.round() as i64 - half_width;
    let mut top = center_y.round() as i64 - half_height;
    if left < 0 {
        left = 0;
    }
    if top < 0 {
        top = 0;
    }
    let max_left = screen_width.saturating_sub(crop_width) as i64;
    let max_top = screen_height.saturating_sub(crop_height) as i64;
    if left > max_left {
        left = max_left;
    }
    if top > max_top {
        top = max_top;
    }

    (left as u32, top as u32, crop_width, crop_height)
}

fn capture_click_crop_screenshot(
    shots_dir: &PathBuf,
    timestamp_ms: u64,
    x: f64,
    y: f64,
) -> Result<String, String> {
    let screens = Screen::all().map_err(|error| format!("Screen list failed: {error}"))?;
    let screen = screens
        .first()
        .ok_or_else(|| "No screen available for capture".to_string())?;
    let image = screen
        .capture()
        .map_err(|error| format!("Screen capture failed: {error}"))?;
    let (left, top, crop_width, crop_height) =
        clamp_crop_bounds(x, y, image.width(), image.height());
    let crop = crop_imm(&image, left, top, crop_width, crop_height).to_image();
    let filename = format!("click-crop-{timestamp_ms}.png");
    let path = shots_dir.join(filename);
    crop.save(&path)
        .map_err(|error| format!("Crop save failed: {error}"))?;
    Ok(path.to_string_lossy().to_string())
}

fn capture_window_crop_screenshot(
    shots_dir: &PathBuf,
    timestamp_ms: u64,
) -> Result<(String, bool), String> {
    let path = capture_full_screenshot(shots_dir, "window", timestamp_ms)?;
    Ok((path, true))
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
                    let window_result = shots_dir
                        .as_ref()
                        .ok_or_else(|| "Screenshot directory not initialized".to_string())
                        .and_then(|dir| capture_window_crop_screenshot(dir, click.timestamp_ms));
                    match window_result {
                        Ok((path, fallback)) => {
                            click.window_crop_path = Some(path);
                            click.window_crop_fallback = fallback;
                        }
                        Err(error) => click.window_crop_error = Some(error),
                    }
                    let crop_result = shots_dir
                        .as_ref()
                        .ok_or_else(|| "Screenshot directory not initialized".to_string())
                        .and_then(|dir| {
                            capture_click_crop_screenshot(dir, click.timestamp_ms, click.x, click.y)
                        });
                    match crop_result {
                        Ok(path) => click.click_crop_path = Some(path),
                        Err(error) => click.click_crop_error = Some(error),
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
                    let window_result = shots_dir
                        .as_ref()
                        .ok_or_else(|| "Screenshot directory not initialized".to_string())
                        .and_then(|dir| {
                            capture_window_crop_screenshot(dir, key_event.timestamp_ms)
                        });
                    match window_result {
                        Ok((path, fallback)) => {
                            key_event.window_crop_path = Some(path);
                            key_event.window_crop_fallback = fallback;
                        }
                        Err(error) => key_event.window_crop_error = Some(error),
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
                        window_crop_path: None,
                        window_crop_error: None,
                        window_crop_fallback: false,
                        click_crop_path: None,
                        click_crop_error: None,
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
                    window_crop_path: None,
                    window_crop_error: None,
                    window_crop_fallback: false,
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
