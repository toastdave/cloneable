use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::events::RecordingSession;

/// Create the directory structure for a new recording session.
///
/// Layout:
/// ```text
/// {base_dir}/recordings/{session_id}/
///   recording.json   (written on stop)
///   shots/           (screenshots saved here during recording)
/// ```
///
/// Returns the absolute path to the session directory.
pub fn create_session_dir(base_dir: &Path, session_id: &str) -> Result<PathBuf, AppError> {
    let session_dir = base_dir.join("recordings").join(session_id);
    let shots_dir = session_dir.join("shots");
    fs::create_dir_all(&shots_dir)?;
    Ok(session_dir)
}

/// Return the absolute path to the `shots/` subdirectory for a session.
pub fn shots_dir(session_dir: &Path) -> PathBuf {
    session_dir.join("shots")
}

/// Write the recording session to `recording.json` inside the session directory.
///
/// Uses a write-to-temp + rename pattern for atomicity.
pub fn save_recording(session_dir: &Path, session: &RecordingSession) -> Result<(), AppError> {
    let json_path = session_dir.join("recording.json");
    let tmp_path = session_dir.join("recording.json.tmp");

    let json = serde_json::to_string_pretty(session)?;
    fs::write(&tmp_path, json)?;
    fs::rename(&tmp_path, &json_path)?;

    Ok(())
}
