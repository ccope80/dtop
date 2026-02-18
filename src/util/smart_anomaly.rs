use crate::models::smart::SmartData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// ATA attribute IDs we watch for non-zero raw values.
const WATCH_ATTRS: &[u32] = &[
    5,   // Reallocated_Sector_Ct
    197, // Current_Pending_Sector
    198, // Offline_Uncorrectable
    199, // UDMA_CRC_Error_Count
];

/// First-seen record for one bad attribute on one device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyRecord {
    pub attr_id:     u32,
    pub attr_name:   String,
    pub first_seen:  i64,    // Unix timestamp when raw_value first went > 0
    pub first_value: u64,    // raw_value at first detection
    pub last_value:  u64,    // raw_value at most recent poll
}

/// Maps attr_id → AnomalyRecord for a single device.
pub type DeviceAnomalies = HashMap<u32, AnomalyRecord>;

/// Maps device name → DeviceAnomalies; persisted to disk.
pub type AnomalyLog = HashMap<String, DeviceAnomalies>;

pub fn anomaly_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("dtop").join("smart_anomalies.json"))
}

pub fn load() -> AnomalyLog {
    let path = match anomaly_path() {
        Some(p) => p,
        None    => return AnomalyLog::new(),
    };
    let text = match fs::read_to_string(&path) {
        Ok(t)  => t,
        Err(_) => return AnomalyLog::new(),
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(log: &AnomalyLog) {
    let path = match anomaly_path() {
        Some(p) => p,
        None    => return,
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(log) {
        let _ = fs::write(&path, text);
    }
}

/// Update anomaly log for a device after a new SMART poll.
/// Returns true if any record was added or updated.
pub fn update(log: &mut AnomalyLog, device_name: &str, smart: &SmartData) -> bool {
    let now = chrono::Local::now().timestamp();
    let device_log = log.entry(device_name.to_string()).or_default();
    let mut changed = false;

    // ATA watched attributes
    for attr in &smart.attributes {
        if !WATCH_ATTRS.contains(&attr.id) || attr.raw_value == 0 {
            continue;
        }
        if let Some(rec) = device_log.get_mut(&attr.id) {
            if attr.raw_value != rec.last_value {
                rec.last_value = attr.raw_value;
                changed = true;
            }
        } else {
            device_log.insert(attr.id, AnomalyRecord {
                attr_id:     attr.id,
                attr_name:   attr.name.clone(),
                first_seen:  now,
                first_value: attr.raw_value,
                last_value:  attr.raw_value,
            });
            changed = true;
        }
    }

    // NVMe media errors (use sentinel ID 9999)
    if let Some(nvme) = &smart.nvme {
        if nvme.media_errors > 0 {
            if let Some(rec) = device_log.get_mut(&9999) {
                if nvme.media_errors != rec.last_value {
                    rec.last_value = nvme.media_errors;
                    changed = true;
                }
            } else {
                device_log.insert(9999, AnomalyRecord {
                    attr_id:     9999,
                    attr_name:   "NVMe Media Errors".to_string(),
                    first_seen:  now,
                    first_value: nvme.media_errors,
                    last_value:  nvme.media_errors,
                });
                changed = true;
            }
        }
    }

    changed
}

/// Format a Unix timestamp as a short date string.
pub fn fmt_ts(ts: i64) -> String {
    use chrono::{TimeZone, Local};
    Local.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
