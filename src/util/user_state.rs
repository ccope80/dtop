use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Lightweight per-user UI state persisted across sessions.
/// Stored at ~/.local/share/dtop/state.json.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UserState {
    /// Name of the active color theme (e.g. "Dracula").  Empty = use default.
    #[serde(default)]
    pub theme_name: String,

    /// Dashboard layout preset index (0=Full, 1=IO-Focus, 2=Storage).
    #[serde(default)]
    pub layout_preset: usize,
}

impl UserState {
    fn path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|p| p.join("dtop").join("state.json"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, s);
            }
        }
    }
}
