use std::collections::HashMap;
use anyhow::Result;

/// Raw snapshot of one line from /proc/diskstats.
#[derive(Debug, Clone, Default)]
pub struct RawDiskstat {
    pub reads_completed:  u64,
    pub sectors_read:     u64,
    pub ms_reading:       u64,
    pub writes_completed: u64,
    pub sectors_written:  u64,
    pub ms_writing:       u64,
    pub ios_in_progress:  u64,
    pub ms_io:            u64,   // "utilisation" counter
}

/// Computed rates for one device over one tick interval.
#[derive(Debug, Clone, Default)]
pub struct DeviceIO {
    pub read_bytes_per_sec:   f64,
    pub write_bytes_per_sec:  f64,
    pub read_iops:            f64,
    pub write_iops:           f64,
    pub io_util_pct:          f64,
    pub queue_depth:          u64,
    pub avg_read_latency_ms:  f64,   // average ms per completed read op
    pub avg_write_latency_ms: f64,   // average ms per completed write op
}

/// Read /proc/diskstats and return a map of device-name → raw snapshot.
pub fn read_diskstats() -> Result<HashMap<String, RawDiskstat>> {
    let content = std::fs::read_to_string("/proc/diskstats")?;
    let mut map = HashMap::new();

    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 { continue; }

        let name = fields[2];
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("zram")
        {
            continue;
        }
        if is_partition(name) {
            continue;
        }

        let entry = RawDiskstat {
            reads_completed:  parse(fields[3]),
            sectors_read:     parse(fields[5]),
            ms_reading:       parse(fields[6]),
            writes_completed: parse(fields[7]),
            sectors_written:  parse(fields[9]),
            ms_writing:       parse(fields[10]),
            ios_in_progress:  parse(fields[11]),
            ms_io:            parse(fields[12]),
        };
        map.insert(name.to_string(), entry);
    }
    Ok(map)
}

/// Compute delta rates given two raw snapshots and the elapsed seconds.
pub fn compute_io(
    prev: &RawDiskstat,
    curr: &RawDiskstat,
    elapsed_sec: f64,
    queue_depth: u64,
) -> DeviceIO {
    if elapsed_sec <= 0.0 {
        return DeviceIO::default();
    }

    let d_reads  = curr.reads_completed .saturating_sub(prev.reads_completed);
    let d_writes = curr.writes_completed.saturating_sub(prev.writes_completed);
    let d_sec_r  = curr.sectors_read    .saturating_sub(prev.sectors_read);
    let d_sec_w  = curr.sectors_written .saturating_sub(prev.sectors_written);
    let d_ms_io  = curr.ms_io           .saturating_sub(prev.ms_io);
    let d_ms_r   = curr.ms_reading      .saturating_sub(prev.ms_reading);
    let d_ms_w   = curr.ms_writing      .saturating_sub(prev.ms_writing);

    let elapsed_ms = elapsed_sec * 1000.0;

    DeviceIO {
        read_bytes_per_sec:   (d_sec_r as f64 * 512.0) / elapsed_sec,
        write_bytes_per_sec:  (d_sec_w as f64 * 512.0) / elapsed_sec,
        read_iops:            d_reads  as f64 / elapsed_sec,
        write_iops:           d_writes as f64 / elapsed_sec,
        io_util_pct:          (d_ms_io as f64 / elapsed_ms * 100.0).min(100.0),
        queue_depth,
        avg_read_latency_ms:  if d_reads  > 0 { d_ms_r as f64 / d_reads  as f64 } else { 0.0 },
        avg_write_latency_ms: if d_writes > 0 { d_ms_w as f64 / d_writes as f64 } else { 0.0 },
    }
}

fn parse(s: &str) -> u64 {
    s.parse().unwrap_or(0)
}

/// Returns true for partition entries like sda1, nvme0n1p1, sdb3.
fn is_partition(name: &str) -> bool {
    if name.starts_with("nvme") {
        return name.contains('p') && name[name.rfind('p').unwrap()..].len() > 1
            && name[name.rfind('p').unwrap() + 1..].chars().all(|c| c.is_ascii_digit());
    }
    let has_leading_alpha = name.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false);
    if has_leading_alpha {
        let rest: String = name.chars().skip_while(|c| c.is_alphabetic()).collect();
        if !rest.is_empty() {
            if name.starts_with("md") || name.starts_with("dm-") {
                return false;
            }
            return rest.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}
