use crate::models::volume::ZfsPool;
use std::collections::HashMap;
use std::process::Command;

/// Try to collect ZFS pool list. Returns empty vec if ZFS not installed.
pub fn read_zpools() -> Vec<ZfsPool> {
    let out = match Command::new("zpool")
        .args(["list", "-Hp", "-o", "name,size,alloc,free,health"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if !out.status.success() { return Vec::new(); }

    let text = String::from_utf8_lossy(&out.stdout);
    let scrub_map = read_scrub_statuses();

    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 5 { return None; }
            let name = f[0].to_string();
            let scrub_status = scrub_map.get(&name).cloned()
                .unwrap_or_else(|| "no scrub".to_string());
            Some(ZfsPool {
                name:         name,
                size_bytes:   f[1].parse().unwrap_or(0),
                alloc_bytes:  f[2].parse().unwrap_or(0),
                free_bytes:   f[3].parse().unwrap_or(0),
                health:       f[4].trim().to_string(),
                scrub_status,
            })
        })
        .collect()
}

/// Run `zpool status` once and extract a short scrub description per pool.
fn read_scrub_statuses() -> HashMap<String, String> {
    let out = match Command::new("zpool").arg("status").output() {
        Ok(o) if o.status.success() => o,
        _ => return HashMap::new(),
    };

    let text = String::from_utf8_lossy(&out.stdout);
    let mut map: HashMap<String, String> = HashMap::new();
    let mut current_pool: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("pool:") {
            current_pool = Some(name.trim().to_string());
        } else if let Some(scan_val) = trimmed.strip_prefix("scan:") {
            if let Some(pool) = &current_pool {
                map.insert(pool.clone(), parse_scan_line(scan_val.trim()));
            }
        }
    }

    map
}

/// Convert a raw "scan:" value into a short human-readable string.
fn parse_scan_line(scan: &str) -> String {
    if scan.starts_with("scrub in progress") {
        // Extract percentage if present: "X% done"
        if let Some(pct) = extract_pct(scan) {
            return format!("scrubbing {:.1}%", pct);
        }
        return "scrubbing…".to_string();
    }
    if scan.starts_with("scrub repaired") || scan.starts_with("scrub canceled") {
        // "scrub repaired 0B in 00:00:01 with 0 errors on Sun Feb  9 00:25:01 2026"
        // Extract short date: last word-group that looks like "YYYY"
        let status = if scan.starts_with("scrub canceled") { "canceled" } else { "ok" };
        if let Some(date) = extract_short_date(scan) {
            return format!("{} ({})", status, date);
        }
        return status.to_string();
    }
    if scan == "none requested" || scan.is_empty() {
        return "no scrub".to_string();
    }
    // Fallback: truncate to 24 chars
    scan.chars().take(24).collect()
}

fn extract_pct(s: &str) -> Option<f64> {
    // Find "NN.NN% done"
    for part in s.split_whitespace() {
        let stripped = part.strip_suffix('%').unwrap_or(part);
        if let Ok(v) = stripped.parse::<f64>() {
            return Some(v);
        }
    }
    None
}

fn extract_short_date(s: &str) -> Option<String> {
    // Grab last token that's a 4-digit year, then also grab the preceding month+day
    let words: Vec<&str> = s.split_whitespace().collect();
    // Find the index of a 4-digit year token
    let year_idx = words.iter().rposition(|w| {
        w.len() == 4 && w.chars().all(|c| c.is_ascii_digit())
    })?;
    // We want "Mon DD YYYY" — year_idx should be >= 2
    if year_idx >= 2 {
        let month = words[year_idx - 2];
        let day   = words[year_idx - 1].trim_start_matches('0');
        let year  = words[year_idx];
        Some(format!("{} {} {}", month, day, year))
    } else {
        Some(words[year_idx].to_string())
    }
}
