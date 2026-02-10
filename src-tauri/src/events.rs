use serde::{Deserialize, Serialize};

/// A complete recording session containing all captured steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSession {
    pub id: String,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub steps: Vec<RecordedStep>,
}

/// A single recorded step corresponding to one input event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedStep {
    pub index: usize,
    pub timestamp: String,
    pub event_type: EventKind,
    pub coordinates: Option<Coordinates>,
    pub key_data: Option<KeyData>,
    pub screenshots: StepScreenshots,
}

/// The kind of input event that was captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Click,
    Keypress,
}

/// Screen coordinates where a click occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    pub x: f64,
    pub y: f64,
}

/// Keyboard event metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyData {
    /// Debug representation of the rdev Key variant.
    pub key: String,
    /// The printable character, if available from the OS layout.
    pub character: Option<String>,
}

/// Paths to the three screenshots captured for a step.
/// Paths are relative to the session directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepScreenshots {
    pub full_screen: String,
    pub window_crop: String,
    pub click_crop: String,
    pub window_crop_fallback: bool,
}

/// Internal event sent from the rdev listener thread to the processor thread.
/// Not serialized to disk — converted into RecordedStep by the processor.
#[derive(Debug, Clone)]
pub struct RawInputEvent {
    pub timestamp: String,
    pub kind: EventKind,
    pub coordinates: Option<Coordinates>,
    pub key_data: Option<KeyData>,
}
