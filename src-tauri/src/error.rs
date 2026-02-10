use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Wayland is not supported. Please run this app under an X11 session (e.g. select 'GNOME on Xorg' or 'Plasma (X11)' at login).")]
    WaylandNotSupported,
    #[error("Already recording")]
    AlreadyRecording,
    #[error("Not recording")]
    NotRecording,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Screenshot error: {0}")]
    Screenshot(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
