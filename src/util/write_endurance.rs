use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEndurance {
    pub total_bytes_written: u64,  // cumulative bytes written since tracking began
    pub first_tracked_at:    i64,  // Unix timestamp when tracking started
}

pub type EnduranceMap = HashMap<String, DeviceEndurance>;

fn endurance_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("dtop").join("write_endurance.json"))
}

pub fn load() -> EnduranceMap {
    let path = match endurance_path() { Some(p) => p, None => return HashMap::new() };
    let text = match fs::read_to_string(&path) { Ok(t) => t, Err(_) => return HashMap::new() };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn update(map: &mut EnduranceMap, device: &str, write_bps: f64, elapsed_secs: f64) {
    if write_bps <= 0.0 || elapsed_secs <= 0.0 { return; }
    let entry = map.entry(device.to_string()).or_insert_with(|| DeviceEndurance {
        total_bytes_written: 0,
        first_tracked_at:    chrono::Local::now().timestamp(),
    });
    entry.total_bytes_written = entry.total_bytes_written
        .saturating_add((write_bps * elapsed_secs) as u64);
}

pub fn save(map: &EnduranceMap) {
    let path = match endurance_path() { Some(p) => p, None => return };
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    if let Ok(json) = serde_json::to_string(map) { let _ = fs::write(path, json); }
}

/// Return the average daily write rate in bytes/day, and how many days have been tracked.
pub fn daily_avg(e: &DeviceEndurance) -> (f64, f64) {
    let now = chrono::Local::now().timestamp();
    let secs_tracked = (now - e.first_tracked_at).max(1) as f64;
    let days_tracked = secs_tracked / 86_400.0;
    let daily = e.total_bytes_written as f64 / days_tracked;
    (daily, days_tracked)
}
