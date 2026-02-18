use crate::alerts::Alert;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub fn log_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("dtop").join("alerts.log"))
}

/// Append a slice of new alerts to the persistent log file.
pub fn append(alerts: &[Alert]) {
    if alerts.is_empty() { return; }
    let path = match log_path() {
        Some(p) => p,
        None    => return,
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        for alert in alerts {
            let _ = writeln!(
                file,
                "{} [{}] {}{}",
                now, alert.severity.label(), alert.prefix(), alert.message
            );
        }
    }
}
