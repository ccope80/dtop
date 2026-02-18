use crate::models::process::{ProcessIORates, RawProcessIO};
use std::collections::HashMap;
use std::fs;

/// Read /proc/<pid>/io for every process we can access.
/// Returns a map of pid → raw cumulative byte counters.
pub fn read_all() -> HashMap<u32, RawProcessIO> {
    let mut map = HashMap::new();

    let dir = match fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return map,
    };

    for entry in dir.flatten() {
        let name = entry.file_name();
        let pid: u32 = match name.to_string_lossy().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let io_path = format!("/proc/{}/io", pid);
        let content = match fs::read_to_string(&io_path) {
            Ok(c) => c,
            Err(_) => continue,  // permission denied or process gone
        };

        let mut read_bytes  = 0u64;
        let mut write_bytes = 0u64;
        for line in content.lines() {
            if let Some(v) = line.strip_prefix("read_bytes: ")  { read_bytes  = v.trim().parse().unwrap_or(0); }
            if let Some(v) = line.strip_prefix("write_bytes: ") { write_bytes = v.trim().parse().unwrap_or(0); }
        }

        let comm = fs::read_to_string(format!("/proc/{}/comm", pid))
            .unwrap_or_default()
            .trim()
            .to_string();

        let uid = read_uid(pid).unwrap_or(0);

        map.insert(pid, RawProcessIO { pid, comm, uid, read_bytes, write_bytes });
    }
    map
}

/// Compute per-second rates from two snapshots.
pub fn compute_rates(
    prev: &HashMap<u32, RawProcessIO>,
    curr: &HashMap<u32, RawProcessIO>,
    elapsed_sec: f64,
    uid_cache: &mut HashMap<u32, String>,
) -> Vec<ProcessIORates> {
    let mut rates = Vec::new();

    for (pid, c) in curr {
        if let Some(p) = prev.get(pid) {
            let dr = c.read_bytes .saturating_sub(p.read_bytes);
            let dw = c.write_bytes.saturating_sub(p.write_bytes);
            if dr == 0 && dw == 0 { continue; }

            let username = uid_cache
                .entry(c.uid)
                .or_insert_with(|| lookup_username(c.uid))
                .clone();

            rates.push(ProcessIORates {
                pid:           *pid,
                comm:          c.comm.clone(),
                username,
                read_per_sec:  dr as f64 / elapsed_sec,
                write_per_sec: dw as f64 / elapsed_sec,
            });
        }
    }

    rates
}

fn read_uid(pid: u32) -> Option<u32> {
    let content = fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Uid:\t") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

fn lookup_username(uid: u32) -> String {
    if let Ok(content) = fs::read_to_string("/etc/passwd") {
        for line in content.lines() {
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            if parts.len() >= 3 && parts[2].parse::<u32>().ok() == Some(uid) {
                return parts[0].to_string();
            }
        }
    }
    uid.to_string()
}
