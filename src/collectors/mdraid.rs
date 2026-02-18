use crate::models::volume::RaidArray;
use std::fs;

/// Parse /proc/mdstat and return a list of md arrays.
pub fn read_mdstat() -> Vec<RaidArray> {
    let content = match fs::read_to_string("/proc/mdstat") {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut arrays = Vec::new();
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        // Each array starts with "mdX : "
        if !line.starts_with("md") || !line.contains(" : ") { continue; }

        let parts: Vec<&str> = line.splitn(2, " : ").collect();
        if parts.len() < 2 { continue; }

        let name   = parts[0].trim().to_string();
        let rest   = parts[1];

        // e.g. "active raid1 sda1[0] sdb1[1]"
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        let state = tokens.get(0).unwrap_or(&"unknown").to_string();

        let level = tokens.iter()
            .find(|t| t.starts_with("raid") || **t == "linear" || **t == "multipath")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Member disks: entries like "sda1[0]" or "sdb1[2](F)"
        let members: Vec<String> = tokens.iter()
            .filter(|t| t.contains('['))
            .map(|t| {
                // strip "[index]" and optional "(F)" suffix
                let end = t.find('[').unwrap_or(t.len());
                t[..end].to_string()
            })
            .collect();

        // Next line has block count and status bitmap like [4/4] [UUUU]
        let detail_line = lines.peek().copied().unwrap_or("").trim().to_string();
        if detail_line.starts_with(char::is_numeric) || detail_line.starts_with("      ") {
            lines.next(); // consume it
        }

        // Parse total blocks from detail line: "976762584 blocks ..."
        let blocks: u64 = detail_line
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let capacity_bytes = blocks * 512;

        // Parse [N/N] — total/active members
        let bitmap = detail_line
            .find('[')
            .and_then(|i| detail_line[i..].find(']').map(|j| &detail_line[i..=i+j]))
            .unwrap_or("[?]")
            .to_string();

        // Rebuild progress line: "      [======>.....] recovery = 50.2%"
        let rebuild_pct = if let Some(next) = lines.peek() {
            if next.contains("recovery =") || next.contains("resync =") || next.contains("check =") {
                let pct_str = next.split('=')
                    .nth(1)
                    .and_then(|s| s.trim().split('%').next())
                    .and_then(|s| s.trim().parse::<f64>().ok());
                lines.next();
                pct_str
            } else {
                None
            }
        } else {
            None
        };

        let degraded = bitmap.contains('_');

        arrays.push(RaidArray {
            name,
            state,
            level,
            members,
            capacity_bytes,
            bitmap,
            degraded,
            rebuild_pct,
        });
    }

    arrays
}
