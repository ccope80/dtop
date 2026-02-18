use crate::models::smart::SmartData;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineAttr {
    pub id:        u32,
    pub name:      String,
    pub raw_value: u64,
    pub value:     u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub device:         String,
    pub saved_at:       i64,    // Unix timestamp
    pub saved_date:     String, // "YYYY-MM-DD" for display
    pub power_on_hours: Option<u64>,
    pub attributes:     Vec<BaselineAttr>,
}

impl Baseline {
    /// Get the delta for an attribute by ID: (baseline_raw, current_raw, delta).
    pub fn attr_delta(&self, id: u32, current_raw: u64) -> Option<(u64, i64)> {
        let base = self.attributes.iter().find(|a| a.id == id)?;
        let delta = current_raw as i64 - base.raw_value as i64;
        Some((base.raw_value, delta))
    }
}

pub fn baseline_path(device_name: &str) -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| {
        p.join("dtop").join("baselines").join(format!("{}.json", device_name))
    })
}

pub fn save(device_name: &str, smart: &SmartData) {
    let path = match baseline_path(device_name) {
        Some(p) => p,
        None    => return,
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let baseline = Baseline {
        device:     device_name.to_string(),
        saved_at:   chrono::Local::now().timestamp(),
        saved_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
        power_on_hours: smart.power_on_hours,
        attributes: smart.attributes.iter().map(|a| BaselineAttr {
            id:        a.id,
            name:      a.name.clone(),
            raw_value: a.raw_value,
            value:     a.value,
        }).collect(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&baseline) {
        let _ = fs::write(path, json);
    }
}

pub fn load(device_name: &str) -> Option<Baseline> {
    let path = baseline_path(device_name)?;
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}
