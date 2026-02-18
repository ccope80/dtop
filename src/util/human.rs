/// Format bytes/s into a human-readable string: "12.5 MB/s"
pub fn fmt_rate(bytes_per_sec: f64) -> String {
    fmt_bytes_f(bytes_per_sec) + "/s"
}

/// Format a raw byte count into a human-readable string: "12.5 MB"
pub fn fmt_bytes(bytes: u64) -> String {
    fmt_bytes_f(bytes as f64)
}

fn fmt_bytes_f(b: f64) -> String {
    const TB: f64 = 1_099_511_627_776.0;
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    const KB: f64 = 1_024.0;
    if b >= TB      { format!("{:.1} TB", b / TB) }
    else if b >= GB { format!("{:.1} GB", b / GB) }
    else if b >= MB { format!("{:.1} MB", b / MB) }
    else if b >= KB { format!("{:.1} KB", b / KB) }
    else            { format!("{:.0} B",  b) }
}

/// Format IOPS: "1,247"
pub fn fmt_iops(iops: f64) -> String {
    let v = iops as u64;
    if v >= 1_000_000 { format!("{:.1}M", v as f64 / 1_000_000.0) }
    else if v >= 1_000 { format!("{:.1}K", v as f64 / 1_000.0) }
    else { format!("{}", v) }
}

/// Format a percentage with one decimal: "84.5%"
pub fn fmt_pct(pct: f64) -> String {
    format!("{:.0}%", pct)
}
