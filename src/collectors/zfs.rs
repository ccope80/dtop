use crate::models::volume::ZfsPool;
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
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 5 { return None; }
            Some(ZfsPool {
                name:        f[0].to_string(),
                size_bytes:  f[1].parse().unwrap_or(0),
                alloc_bytes: f[2].parse().unwrap_or(0),
                free_bytes:  f[3].parse().unwrap_or(0),
                health:      f[4].trim().to_string(),
            })
        })
        .collect()
}
