use crate::models::smart::SmartData;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub type SmartCache = HashMap<String, SmartData>;

pub fn cache_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("dtop").join("smart_cache.json"))
}

pub fn load() -> SmartCache {
    let path = match cache_path() {
        Some(p) => p,
        None    => return SmartCache::new(),
    };
    let text = match fs::read_to_string(&path) {
        Ok(t)  => t,
        Err(_) => return SmartCache::new(),
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(cache: &SmartCache) {
    let path = match cache_path() {
        Some(p) => p,
        None    => return,
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(cache) {
        let _ = fs::write(&path, text);
    }
}
