use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const MAX_ENTRIES: usize = 90; // ~90 SMART polls ≈ 7.5 days at 5 min each

/// Per-device health score history: device_name → [score, ...] (oldest first, newest last)
pub type HealthHistory = HashMap<String, Vec<u8>>;

#[derive(Debug, Serialize, Deserialize, Default)]
struct Persisted {
    entries: HashMap<String, Vec<u8>>,
}

fn history_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("dtop").join("health_history.json"))
}

pub fn load() -> HealthHistory {
    let path = match history_path() { Some(p) => p, None => return HashMap::new() };
    let text = match fs::read_to_string(&path) { Ok(t) => t, Err(_) => return HashMap::new() };
    serde_json::from_str::<Persisted>(&text)
        .map(|p| p.entries)
        .unwrap_or_default()
}

pub fn append(history: &mut HealthHistory, device: &str, score: u8) {
    let v = history.entry(device.to_string()).or_default();
    v.push(score);
    if v.len() > MAX_ENTRIES {
        let drain = v.len() - MAX_ENTRIES;
        v.drain(..drain);
    }
}

pub fn save(history: &HealthHistory) {
    let path = match history_path() { Some(p) => p, None => return };
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    let p = Persisted { entries: history.clone() };
    if let Ok(json) = serde_json::to_string(&p) { let _ = fs::write(path, json); }
}
