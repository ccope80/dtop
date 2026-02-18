use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

fn ack_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("dtop").join("acked_alerts.json"))
}

/// Load persisted acked alert keys from disk.
/// Returns an empty set if the file doesn't exist or can't be parsed.
pub fn load() -> HashSet<String> {
    let path = match ack_path() { Some(p) => p, None => return HashSet::new() };
    let text = match fs::read_to_string(&path) { Ok(t) => t, Err(_) => return HashSet::new() };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Persist the current acked alert key set to disk (best-effort).
pub fn save(acked: &HashSet<String>) {
    let path = match ack_path() { Some(p) => p, None => return };
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    if let Ok(json) = serde_json::to_string(acked) { let _ = fs::write(path, json); }
}
