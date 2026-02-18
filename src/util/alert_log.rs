use crate::alerts::{Alert, Severity};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

pub fn log_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("dtop").join("alerts.log"))
}

/// Load the last `n` entries from the alert log for pre-populating alert history.
/// Returns `(ts_str, Alert)` pairs, oldest first (same order as the file).
/// Lines that can't be parsed are skipped silently.
pub fn load_recent(n: usize) -> Vec<(String, Alert)> {
    let path = match log_path() { Some(p) => p, None => return Vec::new() };
    let file = match fs::File::open(&path) { Ok(f) => f, Err(_) => return Vec::new() };

    // Read all lines, keep last n
    let lines: Vec<String> = BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .collect();
    let start = lines.len().saturating_sub(n);

    lines[start..].iter().filter_map(|line| parse_log_line(line)).collect()
}

// Format: "YYYY-MM-DD HH:MM:SS [CRIT/WARN/INFO] <prefix+message>"
fn parse_log_line(line: &str) -> Option<(String, Alert)> {
    // Timestamp is first 19 chars: "2024-01-15 12:34:56"
    if line.len() < 22 { return None; }
    let ts_str = line[11..19].to_string(); // "HH:MM:SS"
    let rest   = &line[20..];             // "[WARN] [sda] ..."

    let severity = if rest.starts_with("[CRIT]") {
        Severity::Critical
    } else if rest.starts_with("[WARN]") {
        Severity::Warning
    } else {
        Severity::Info
    };

    // Skip "[XXXX] " (7 chars) to get prefix+message
    let msg = rest.get(7..).unwrap_or("").trim().to_string();

    Some((ts_str, Alert {
        severity,
        device: None,
        mount:  None,
        message: msg,
    }))
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
