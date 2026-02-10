use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::unbounded;
use image::imageops::crop_imm;
use rdev::EventType;
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StepEventType {
    Click,
    Key,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ActionType {
    Click,
    Type,
    Wait,
    Assert,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Step {
    id: String,
    event_type: StepEventType,
    #[serde(default)]
    action_type: Option<ActionType>,
    timestamp_ms: u64,
    full_screenshot_path: Option<String>,
    window_crop_path: Option<String>,
    window_crop_fallback: bool,
    click_crop_path: Option<String>,
    #[serde(default)]
    input_text: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Clone, Debug)]
enum InputEvent {
    Click(ClickEvent),
    Key(KeyEvent),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordingSession {
    session_id: String,
    started_at_ms: u64,
    stopped_at_ms: u64,
    click_events: Vec<ClickEvent>,
    key_events: Vec<KeyEvent>,
    #[serde(default)]
    steps: Vec<Step>,
}

struct RecorderState {
    is_recording: bool,
    session_id: Option<String>,
    started_at_ms: Option<u64>,
    recording_dir: Option<PathBuf>,
    shots_dir: Option<PathBuf>,
    click_events: Vec<ClickEvent>,
    key_events: Vec<KeyEvent>,
    listener_error: Option<String>,
}

const CLICK_CROP_WIDTH: u32 = 400;
const CLICK_CROP_HEIGHT: u32 = 300;
const KEY_GROUP_WINDOW_MS: u64 = 500;

impl RecorderState {
    fn new() -> Self {
        Self {
            is_recording: false,
            session_id: None,
            started_at_ms: None,
            recording_dir: None,
            shots_dir: None,
            click_events: Vec::new(),
            key_events: Vec::new(),
            listener_error: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct StopRecordingResult {
    session_id: String,
    click_count: usize,
    key_count: usize,
    listener_error: Option<String>,
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

fn build_key_group_text(group: &[KeyEvent]) -> Option<String> {
    let mut text_parts: Vec<String> = Vec::new();
    let mut key_parts: Vec<String> = Vec::new();
    for event in group {
        if let Some(text) = &event.text {
            if !text.trim().is_empty() {
                text_parts.push(text.clone());
                continue;
            }
        }
        if let Some(key) = &event.key {
            if !key.trim().is_empty() {
                key_parts.push(key.clone());
            }
        }
    }

    if !text_parts.is_empty() {
        let mut combined = text_parts.join("");
        if !key_parts.is_empty() {
            combined.push(' ');
            combined.push_str(&format!("[{}]", key_parts.join(" ")));
        }
        return Some(combined);
    }

    if !key_parts.is_empty() {
        return Some(format!("Keys: {}", key_parts.join(" ")));
    }

    None
}

fn build_key_steps(key_events: &[KeyEvent]) -> Vec<Step> {
    let mut sorted_events = key_events.to_vec();
    sorted_events.sort_by_key(|event| event.timestamp_ms);
    let mut steps = Vec::new();
    let mut current_group: Vec<KeyEvent> = Vec::new();
    let mut group_index = 0;

    for event in sorted_events.into_iter() {
        if let Some(last) = current_group.last() {
            let delta = event.timestamp_ms.saturating_sub(last.timestamp_ms);
            if delta <= KEY_GROUP_WINDOW_MS {
                current_group.push(event);
                continue;
            }
        }

        if !current_group.is_empty() {
            let step = build_key_step(&current_group, group_index);
            steps.push(step);
            group_index += 1;
            current_group.clear();
        }

        current_group.push(event);
    }

    if !current_group.is_empty() {
        steps.push(build_key_step(&current_group, group_index));
    }

    steps
}

fn build_key_step(group: &[KeyEvent], group_index: usize) -> Step {
    let first = group
        .first()
        .expect("Key event group should include at least one event");
    let last = group
        .last()
        .expect("Key event group should include at least one event");

    Step {
        id: format!("key-{}-{}", first.timestamp_ms, group_index),
        event_type: StepEventType::Key,
        action_type: Some(ActionType::Type),
        timestamp_ms: last.timestamp_ms,
        full_screenshot_path: last.full_screenshot_path.clone(),
        window_crop_path: last.window_crop_path.clone(),
        window_crop_fallback: last.window_crop_fallback,
        click_crop_path: None,
        input_text: build_key_group_text(group),
        title: None,
        description: None,
    }
}

fn build_steps(click_events: &[ClickEvent], key_events: &[KeyEvent]) -> Vec<Step> {
    let mut steps = Vec::with_capacity(click_events.len() + key_events.len());
    for (index, event) in click_events.iter().enumerate() {
        steps.push(Step {
            id: format!("click-{}-{}", event.timestamp_ms, index),
            event_type: StepEventType::Click,
            action_type: Some(ActionType::Click),
            timestamp_ms: event.timestamp_ms,
            full_screenshot_path: event.full_screenshot_path.clone(),
            window_crop_path: event.window_crop_path.clone(),
            window_crop_fallback: event.window_crop_fallback,
            click_crop_path: event.click_crop_path.clone(),
            input_text: None,
            title: None,
            description: None,
        });
    }

    steps.extend(build_key_steps(key_events));
    steps.sort_by_key(|step| step.timestamp_ms);
    steps
}

fn normalize_step_action_types(steps: &mut [Step]) {
    for step in steps.iter_mut() {
        if step.action_type.is_some() {
            continue;
        }
        step.action_type = Some(match step.event_type {
            StepEventType::Click => ActionType::Click,
            StepEventType::Key => ActionType::Type,
        });
    }
}

fn write_recording_json(
    recording_dir: &PathBuf,
    recording: &RecordingSession,
) -> Result<(), String> {
    fs::create_dir_all(recording_dir)
        .map_err(|error| format!("Recording dir create failed: {error}"))?;
    let payload = serde_json::to_vec_pretty(recording)
        .map_err(|error| format!("JSON encode failed: {error}"))?;
    let temp_path = recording_dir.join("recording.json.tmp");
    let target_path = recording_dir.join("recording.json");
    fs::write(&temp_path, payload)
        .map_err(|error| format!("Recording JSON write failed: {error}"))?;
    if target_path.exists() {
        fs::remove_file(&target_path)
            .map_err(|error| format!("Recording JSON remove failed: {error}"))?;
    }
    fs::rename(&temp_path, &target_path)
        .map_err(|error| format!("Recording JSON rename failed: {error}"))?;
    Ok(())
}

fn spawn_global_input_listener(state: Arc<Mutex<RecorderState>>) {
    let (sender, receiver) = unbounded::<InputEvent>();
    let state_for_events = Arc::clone(&state);

    thread::spawn(move || {
        for event in receiver.iter() {
            let recorder_state = match state_for_events.lock() {
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

                    let mut recorder_state = match state_for_events.lock() {
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

                    let mut recorder_state = match state_for_events.lock() {
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

    let state_for_listener = Arc::clone(&state);
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
            let message = format!("Global input listener stopped: {error:?}");
            eprintln!("{message}");
            if let Ok(mut recorder_state) = state_for_listener.lock() {
                recorder_state.listener_error = Some(message);
            }
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
    let has_pending = state.session_id.is_some()
        && (!state.click_events.is_empty() || !state.key_events.is_empty());
    if has_pending {
        return Err("Previous recording not saved yet".to_string());
    }

    let session_id = new_session_id();
    let base_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let recording_dir = base_dir.join("recordings").join(&session_id);
    let shots_dir = recording_dir.join("shots");
    if let Err(error) = fs::create_dir_all(&shots_dir) {
        eprintln!("Failed to create recordings dir: {error:?}");
    }
    state.is_recording = true;
    state.session_id = Some(session_id.clone());
    state.started_at_ms = Some(now_millis());
    state.recording_dir = Some(recording_dir);
    state.shots_dir = Some(shots_dir);
    state.click_events.clear();
    state.key_events.clear();
    state.listener_error = None;

    Ok(session_id)
}

#[tauri::command]
fn stop_recording(
    state: tauri::State<Arc<Mutex<RecorderState>>>,
) -> Result<StopRecordingResult, String> {
    let mut recorder_state = state
        .lock()
        .map_err(|_| "Recording state lock poisoned".to_string())?;

    let has_pending = recorder_state.session_id.is_some()
        && (!recorder_state.click_events.is_empty() || !recorder_state.key_events.is_empty());
    if !recorder_state.is_recording && !has_pending {
        return Err("Recording is not active".to_string());
    }

    let session_id = recorder_state
        .session_id
        .clone()
        .unwrap_or_else(new_session_id);
    let started_at_ms = recorder_state.started_at_ms.unwrap_or_else(now_millis);
    let recording_dir = recorder_state
        .recording_dir
        .clone()
        .ok_or_else(|| "Recording directory not initialized".to_string())?;
    let click_events = recorder_state.click_events.clone();
    let key_events = recorder_state.key_events.clone();
    let listener_error = recorder_state.listener_error.clone();
    recorder_state.is_recording = false;
    drop(recorder_state);

    let stopped_at_ms = now_millis();
    let click_count = click_events.len();
    let key_count = key_events.len();
    let steps = build_steps(&click_events, &key_events);
    let recording = RecordingSession {
        session_id: session_id.clone(),
        started_at_ms,
        stopped_at_ms,
        click_events,
        key_events,
        steps,
    };
    write_recording_json(&recording_dir, &recording)?;

    let mut recorder_state = state
        .lock()
        .map_err(|_| "Recording state lock poisoned".to_string())?;
    recorder_state.is_recording = false;
    recorder_state.session_id = None;
    recorder_state.started_at_ms = None;
    recorder_state.recording_dir = None;
    recorder_state.shots_dir = None;
    recorder_state.click_events.clear();
    recorder_state.key_events.clear();

    Ok(StopRecordingResult {
        session_id,
        click_count,
        key_count,
        listener_error,
    })
}

#[tauri::command]
fn load_recording(
    app_handle: tauri::AppHandle,
    session_id: String,
) -> Result<RecordingSession, String> {
    let base_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let recording_path = base_dir
        .join("recordings")
        .join(&session_id)
        .join("recording.json");
    let payload = fs::read(&recording_path)
        .map_err(|error| format!("Recording JSON read failed: {error}"))?;
    let mut recording = serde_json::from_slice::<RecordingSession>(&payload)
        .map_err(|error| format!("Recording JSON parse failed: {error}"))?;
    if recording.steps.is_empty() {
        recording.steps = build_steps(&recording.click_events, &recording.key_events);
    }
    normalize_step_action_types(&mut recording.steps);
    Ok(recording)
}

#[tauri::command]
fn update_step_annotations(
    app_handle: tauri::AppHandle,
    session_id: String,
    step_id: String,
    title: Option<String>,
    description: Option<String>,
    action_type: Option<ActionType>,
) -> Result<Step, String> {
    let base_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let recording_dir = base_dir.join("recordings").join(&session_id);
    let recording_path = recording_dir.join("recording.json");
    let payload = fs::read(&recording_path)
        .map_err(|error| format!("Recording JSON read failed: {error}"))?;
    let mut recording = serde_json::from_slice::<RecordingSession>(&payload)
        .map_err(|error| format!("Recording JSON parse failed: {error}"))?;
    if recording.steps.is_empty() {
        recording.steps = build_steps(&recording.click_events, &recording.key_events);
    }
    normalize_step_action_types(&mut recording.steps);
    let step = recording
        .steps
        .iter_mut()
        .find(|step| step.id == step_id)
        .ok_or_else(|| "Step not found".to_string())?;
    step.title = title;
    step.description = description;
    if action_type.is_some() {
        step.action_type = action_type;
    }
    let updated_step = step.clone();
    write_recording_json(&recording_dir, &recording)?;
    Ok(updated_step)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let recorder_state = Arc::new(Mutex::new(RecorderState::new()));
    spawn_global_input_listener(recorder_state.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(recorder_state)
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            load_recording,
            update_step_annotations
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
