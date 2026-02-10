use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use chrono::Utc;
use uuid::Uuid;

use crate::error::AppError;
use crate::events::{
    Coordinates, EventKind, KeyData, RawInputEvent, RecordedStep, RecordingSession,
};
use crate::screenshots::capture_step_screenshots;
use crate::storage;

/// Shared recorder state managed by Tauri via `.manage()`.
pub struct RecorderState {
    pub inner: Mutex<RecorderInner>,
}

pub struct RecorderInner {
    pub is_recording: bool,
    /// The session being recorded (populated on start, consumed on stop).
    pub session: Option<RecordingSession>,
    /// Absolute path to the session directory.
    pub session_dir: Option<PathBuf>,
    /// Shared flag that the rdev listener checks; set to false on stop.
    pub recording_flag: Arc<AtomicBool>,
    /// Handle to the processor thread so we can join on stop.
    pub processor_handle: Option<JoinHandle<Vec<RecordedStep>>>,
    /// Handle to the listener thread (kept alive; we don't join it because
    /// rdev::listen blocks indefinitely on Linux).
    pub listener_handle: Option<JoinHandle<()>>,
}

impl RecorderState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(RecorderInner {
                is_recording: false,
                session: None,
                session_dir: None,
                recording_flag: Arc::new(AtomicBool::new(false)),
                processor_handle: None,
                listener_handle: None,
            }),
        }
    }
}

/// Returns true if the current session is running under Wayland.
fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|v| v == "wayland")
        .unwrap_or(false)
        || std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// Start a new recording session.
///
/// - Creates the session directory under `base_dir`.
/// - Spawns the rdev listener thread and the event processor thread.
/// - Returns the session ID (UUID).
pub fn start(state: &RecorderState, base_dir: &PathBuf) -> Result<String, AppError> {
    if is_wayland() {
        return Err(AppError::WaylandNotSupported);
    }

    let mut inner = state.inner.lock().unwrap();

    if inner.is_recording {
        return Err(AppError::AlreadyRecording);
    }

    let session_id = Uuid::new_v4().to_string();
    let session_dir = storage::create_session_dir(base_dir, &session_id)?;
    let shots_dir = storage::shots_dir(&session_dir);

    let session = RecordingSession {
        id: session_id.clone(),
        started_at: Utc::now().to_rfc3339(),
        stopped_at: None,
        steps: Vec::new(),
    };

    // Channel for raw events from listener -> processor.
    let (tx, rx): (Sender<RawInputEvent>, Receiver<RawInputEvent>) = mpsc::channel();

    // Shared recording flag — listener checks this on every event.
    let recording_flag = Arc::new(AtomicBool::new(true));
    inner.recording_flag = recording_flag.clone();

    // --- Spawn the rdev listener thread ---
    let listener_flag = recording_flag.clone();
    let listener_tx = tx.clone();
    // We track the last known mouse position using a shared Mutex.
    let last_mouse_pos: Arc<Mutex<(f64, f64)>> = Arc::new(Mutex::new((0.0, 0.0)));
    let listener_mouse_pos = last_mouse_pos.clone();

    let listener_handle = thread::spawn(move || {
        // rdev::listen blocks forever. The callback filters by the recording flag.
        let mut shift_down = false;
        let mut caps_lock = false;
        let _ = rdev::listen(move |event: rdev::Event| {
            if !listener_flag.load(Ordering::Relaxed) {
                return;
            }

            match event.event_type {
                rdev::EventType::MouseMove { x, y } => {
                    if let Ok(mut pos) = listener_mouse_pos.lock() {
                        *pos = (x, y);
                    }
                }
                rdev::EventType::ButtonPress(_button) => {
                    let (mx, my) = *listener_mouse_pos.lock().unwrap_or_else(|e| e.into_inner());
                    let raw = RawInputEvent {
                        timestamp: Utc::now().to_rfc3339(),
                        kind: EventKind::Click,
                        coordinates: Some(Coordinates { x: mx, y: my }),
                        key_data: None,
                    };
                    let _ = listener_tx.send(raw);
                }
                rdev::EventType::KeyPress(key) => {
                    let (mx, my) = *listener_mouse_pos.lock().unwrap_or_else(|e| e.into_inner());
                    if matches!(key, rdev::Key::ShiftLeft | rdev::Key::ShiftRight) {
                        shift_down = true;
                    }
                    if matches!(key, rdev::Key::CapsLock) {
                        caps_lock = !caps_lock;
                    }
                    let character = if cfg!(target_os = "macos") {
                        key_to_us_char(key, shift_down, caps_lock)
                    } else {
                        event.name.clone()
                    };
                    let raw = RawInputEvent {
                        timestamp: Utc::now().to_rfc3339(),
                        kind: EventKind::Keypress,
                        coordinates: Some(Coordinates { x: mx, y: my }),
                        key_data: Some(KeyData {
                            key: format!("{:?}", key),
                            character,
                        }),
                    };
                    let _ = listener_tx.send(raw);
                }
                rdev::EventType::KeyRelease(key) => {
                    if matches!(key, rdev::Key::ShiftLeft | rdev::Key::ShiftRight) {
                        shift_down = false;
                    }
                }
                _ => {}
            }
        });
    });

    // --- Spawn the processor thread ---
    // This thread receives raw events, captures screenshots, and builds RecordedSteps.
    // It uses recv_timeout so it can check the recording flag and exit cleanly,
    // since the rdev listener thread holds a Sender clone indefinitely.
    let proc_shots_dir = shots_dir.clone();
    let proc_flag = recording_flag.clone();
    let processor_handle = thread::spawn(move || {
        let mut steps: Vec<RecordedStep> = Vec::new();
        let mut step_index: usize = 0;

        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(raw) => {
                    // Determine coordinates for screenshot capture.
                    let (sx, sy) = match &raw.coordinates {
                        Some(coords) => (coords.x, coords.y),
                        None => (0.0, 0.0),
                    };

                    // Capture screenshots (best-effort — log errors but don't crash).
                    let screenshots =
                        match capture_step_screenshots(step_index, sx, sy, &proc_shots_dir) {
                            Ok(result) => result.screenshots,
                            Err(e) => {
                                eprintln!(
                                    "[recorder] screenshot capture error for step {step_index}: {e}"
                                );
                                // Create placeholder paths so the step is still recorded.
                                crate::events::StepScreenshots {
                                    full_screen: String::new(),
                                    window_crop: String::new(),
                                    click_crop: String::new(),
                                    window_crop_fallback: true,
                                }
                            }
                        };

                    let step = RecordedStep {
                        index: step_index,
                        timestamp: raw.timestamp,
                        event_type: raw.kind,
                        coordinates: raw.coordinates,
                        key_data: raw.key_data,
                        screenshots,
                    };

                    println!(
                        "[recorder] captured step {}: {:?}",
                        step_index, step.event_type
                    );
                    steps.push(step);
                    step_index += 1;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // No event within timeout — check if we should stop.
                    if !proc_flag.load(Ordering::Relaxed) {
                        // Recording stopped. Drain any remaining events.
                        while let Ok(raw) = rx.try_recv() {
                            let (sx, sy) = match &raw.coordinates {
                                Some(coords) => (coords.x, coords.y),
                                None => (0.0, 0.0),
                            };

                            let screenshots = match capture_step_screenshots(
                                step_index,
                                sx,
                                sy,
                                &proc_shots_dir,
                            ) {
                                Ok(result) => result.screenshots,
                                Err(e) => {
                                    eprintln!(
                                        "[recorder] screenshot capture error for step {step_index}: {e}"
                                    );
                                    crate::events::StepScreenshots {
                                        full_screen: String::new(),
                                        window_crop: String::new(),
                                        click_crop: String::new(),
                                        window_crop_fallback: true,
                                    }
                                }
                            };

                            let step = RecordedStep {
                                index: step_index,
                                timestamp: raw.timestamp,
                                event_type: raw.kind,
                                coordinates: raw.coordinates,
                                key_data: raw.key_data,
                                screenshots,
                            };

                            steps.push(step);
                            step_index += 1;
                        }
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // All senders dropped — exit.
                    break;
                }
            }
        }

        steps
    });

    inner.is_recording = true;
    inner.session = Some(session);
    inner.session_dir = Some(session_dir);
    inner.processor_handle = Some(processor_handle);
    inner.listener_handle = Some(listener_handle);

    // Drop the original sender so only the listener thread's clone remains.
    // When we stop recording and the flag goes false, the listener stops sending,
    // and then we drop the listener's sender by detaching the thread, closing the channel.
    drop(tx);

    Ok(session_id)
}

/// Stop the current recording session.
///
/// - Signals the listener to stop capturing events.
/// - Waits for the processor to drain remaining events and collects the steps.
/// - Writes `recording.json` to disk.
/// - Returns the absolute path to the session directory.
pub fn stop(state: &RecorderState) -> Result<String, AppError> {
    let mut inner = state.inner.lock().unwrap();

    if !inner.is_recording {
        return Err(AppError::NotRecording);
    }

    // Signal the listener callback to stop sending events.
    inner.recording_flag.store(false, Ordering::Relaxed);

    // Detach the listener thread. On Linux, rdev::listen blocks forever and
    // there's no clean way to stop it. The AtomicBool flag makes the callback
    // a no-op, so it's harmless. The thread will exit when the app closes.
    if let Some(handle) = inner.listener_handle.take() {
        drop(handle);
    }

    // Wait for the processor thread to notice the flag and finish draining.
    // The processor uses recv_timeout + flag check, so it will exit within ~200ms.
    let steps = if let Some(handle) = inner.processor_handle.take() {
        match handle.join() {
            Ok(steps) => steps,
            Err(_) => {
                eprintln!("[recorder] processor thread panicked");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Finalize the session.
    let session_dir = inner
        .session_dir
        .take()
        .expect("session_dir should be set while recording");

    let mut session = inner
        .session
        .take()
        .expect("session should be set while recording");

    session.stopped_at = Some(Utc::now().to_rfc3339());
    session.steps = steps;

    // Write recording.json.
    storage::save_recording(&session_dir, &session)?;

    inner.is_recording = false;

    let session_path = session_dir.to_str().unwrap_or_default().to_string();

    println!(
        "[recorder] session {} saved to {}",
        session.id, session_path
    );

    Ok(session_path)
}

fn key_to_us_char(key: rdev::Key, shift_down: bool, caps_lock: bool) -> Option<String> {
    let is_upper = shift_down ^ caps_lock;
    let letter = match key {
        rdev::Key::KeyA => Some('a'),
        rdev::Key::KeyB => Some('b'),
        rdev::Key::KeyC => Some('c'),
        rdev::Key::KeyD => Some('d'),
        rdev::Key::KeyE => Some('e'),
        rdev::Key::KeyF => Some('f'),
        rdev::Key::KeyG => Some('g'),
        rdev::Key::KeyH => Some('h'),
        rdev::Key::KeyI => Some('i'),
        rdev::Key::KeyJ => Some('j'),
        rdev::Key::KeyK => Some('k'),
        rdev::Key::KeyL => Some('l'),
        rdev::Key::KeyM => Some('m'),
        rdev::Key::KeyN => Some('n'),
        rdev::Key::KeyO => Some('o'),
        rdev::Key::KeyP => Some('p'),
        rdev::Key::KeyQ => Some('q'),
        rdev::Key::KeyR => Some('r'),
        rdev::Key::KeyS => Some('s'),
        rdev::Key::KeyT => Some('t'),
        rdev::Key::KeyU => Some('u'),
        rdev::Key::KeyV => Some('v'),
        rdev::Key::KeyW => Some('w'),
        rdev::Key::KeyX => Some('x'),
        rdev::Key::KeyY => Some('y'),
        rdev::Key::KeyZ => Some('z'),
        _ => None,
    };

    if let Some(ch) = letter {
        let mapped = if is_upper {
            ch.to_ascii_uppercase()
        } else {
            ch
        };
        return Some(mapped.to_string());
    }

    let mapped = match key {
        rdev::Key::Num1 => {
            if shift_down {
                "!"
            } else {
                "1"
            }
        }
        rdev::Key::Num2 => {
            if shift_down {
                "@"
            } else {
                "2"
            }
        }
        rdev::Key::Num3 => {
            if shift_down {
                "#"
            } else {
                "3"
            }
        }
        rdev::Key::Num4 => {
            if shift_down {
                "$"
            } else {
                "4"
            }
        }
        rdev::Key::Num5 => {
            if shift_down {
                "%"
            } else {
                "5"
            }
        }
        rdev::Key::Num6 => {
            if shift_down {
                "^"
            } else {
                "6"
            }
        }
        rdev::Key::Num7 => {
            if shift_down {
                "&"
            } else {
                "7"
            }
        }
        rdev::Key::Num8 => {
            if shift_down {
                "*"
            } else {
                "8"
            }
        }
        rdev::Key::Num9 => {
            if shift_down {
                "("
            } else {
                "9"
            }
        }
        rdev::Key::Num0 => {
            if shift_down {
                ")"
            } else {
                "0"
            }
        }
        rdev::Key::Space => " ",
        rdev::Key::Tab => "\t",
        rdev::Key::Return => "\n",
        rdev::Key::Backspace => "\u{8}",
        rdev::Key::BackQuote => {
            if shift_down {
                "~"
            } else {
                "`"
            }
        }
        rdev::Key::Minus => {
            if shift_down {
                "_"
            } else {
                "-"
            }
        }
        rdev::Key::Equal => {
            if shift_down {
                "+"
            } else {
                "="
            }
        }
        rdev::Key::LeftBracket => {
            if shift_down {
                "{"
            } else {
                "["
            }
        }
        rdev::Key::RightBracket => {
            if shift_down {
                "}"
            } else {
                "]"
            }
        }
        rdev::Key::SemiColon => {
            if shift_down {
                ":"
            } else {
                ";"
            }
        }
        rdev::Key::Quote => {
            if shift_down {
                "\""
            } else {
                "'"
            }
        }
        rdev::Key::BackSlash | rdev::Key::IntlBackslash => {
            if shift_down {
                "|"
            } else {
                "\\"
            }
        }
        rdev::Key::Comma => {
            if shift_down {
                "<"
            } else {
                ","
            }
        }
        rdev::Key::Dot => {
            if shift_down {
                ">"
            } else {
                "."
            }
        }
        rdev::Key::Slash => {
            if shift_down {
                "?"
            } else {
                "/"
            }
        }
        rdev::Key::Kp0 => "0",
        rdev::Key::Kp1 => "1",
        rdev::Key::Kp2 => "2",
        rdev::Key::Kp3 => "3",
        rdev::Key::Kp4 => "4",
        rdev::Key::Kp5 => "5",
        rdev::Key::Kp6 => "6",
        rdev::Key::Kp7 => "7",
        rdev::Key::Kp8 => "8",
        rdev::Key::Kp9 => "9",
        rdev::Key::KpPlus => "+",
        rdev::Key::KpMinus => "-",
        rdev::Key::KpMultiply => "*",
        rdev::Key::KpDivide => "/",
        rdev::Key::KpReturn => "\n",
        rdev::Key::KpDelete => ".",
        _ => return None,
    };

    Some(mapped.to_string())
}
