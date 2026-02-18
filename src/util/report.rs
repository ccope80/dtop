use crate::alerts::Alert;
use crate::collectors::{filesystem, lsblk, smart as smart_collector};
use crate::models::device::BlockDevice;
use crate::models::filesystem::Filesystem;
use crate::util::human::fmt_bytes;

/// Generate a human-readable health report to a String.
pub fn generate(
    devices:     &[BlockDevice],
    filesystems: &[Filesystem],
    alerts:      &[Alert],
) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let mut out = String::new();

    out.push_str(&format!("═══════════════════════════════════════════════\n"));
    out.push_str(&format!("  DTop Health Report — {}\n", now));
    out.push_str(&format!("═══════════════════════════════════════════════\n\n"));

    // ── Active alerts ──────────────────────────────────────────────────
    out.push_str(&format!("── Active Alerts ({}) ─────────────────────────\n", alerts.len()));
    if alerts.is_empty() {
        out.push_str("  ● All systems nominal\n");
    } else {
        for a in alerts {
            out.push_str(&format!("  [{}]  {}{}\n", a.severity.label(), a.prefix(), a.message));
        }
    }
    out.push('\n');

    // ── Devices ────────────────────────────────────────────────────────
    out.push_str(&format!("── Block Devices ({}) ─────────────────────────\n", devices.len()));
    for dev in devices {
        let model   = dev.model.as_deref().unwrap_or("Unknown");
        let serial  = dev.serial.as_deref().unwrap_or("—");
        let cap     = fmt_bytes(dev.capacity_bytes);
        let smart_s = match &dev.smart {
            Some(s) => s.status.label().trim().to_string(),
            None    => "?".to_string(),
        };
        let temp = match dev.temperature() {
            Some(t) => format!("{}°C", t),
            None    => "—".to_string(),
        };
        out.push_str(&format!(
            "  {:8}  {:4}  SMART:{:5}  Temp:{:6}  Cap:{:10}  {}\n  Serial: {}\n",
            dev.name, dev.dev_type.label().trim(), smart_s, temp, cap, model, serial
        ));

        // NVMe endurance
        if let Some(smart) = &dev.smart {
            if let Some(nvme) = &smart.nvme {
                out.push_str(&format!(
                    "  Endurance: {}% used  |  Written: {}  |  POH: {} h\n",
                    nvme.percentage_used,
                    fmt_bytes(nvme.bytes_written()),
                    nvme.power_on_hours,
                ));
            } else if let Some(poh) = smart.power_on_hours {
                out.push_str(&format!("  Power On Hours: {} h\n", poh));
            }
        }
        out.push('\n');
    }

    // ── Filesystems ────────────────────────────────────────────────────
    out.push_str(&format!("── Filesystems ({}) ───────────────────────────\n", filesystems.len()));
    out.push_str(&format!(
        "  {:<20} {:<8} {:>10} {:>10} {:>10} {:>6}\n",
        "Mount", "FS", "Total", "Used", "Avail", "Use%"
    ));
    out.push_str(&format!("  {}\n", "─".repeat(68)));
    for fs in filesystems {
        out.push_str(&format!(
            "  {:<20} {:<8} {:>10} {:>10} {:>10} {:>5.1}%\n",
            fs.mount, fs.fs_type,
            fmt_bytes(fs.total_bytes), fmt_bytes(fs.used_bytes),
            fmt_bytes(fs.avail_bytes), fs.use_pct(),
        ));
    }
    out.push('\n');

    out.push_str("═══════════════════════════════════════════════\n");
    out
}

/// Collect a one-shot snapshot via lsblk + smartctl (background) and return
/// a populated pair of (devices, filesystems) suitable for report generation.
/// SMART data is skipped (would require waiting for background polls).
pub fn collect_snapshot() -> (Vec<BlockDevice>, Vec<Filesystem>) {
    use crate::collectors::diskstats;
    use crate::models::device::BlockDevice;

    let lsblk_devs = lsblk::run_lsblk().unwrap_or_default();
    let raw_stats  = diskstats::read_diskstats().unwrap_or_default();
    let fs_list    = filesystem::read_filesystems().unwrap_or_default();

    let mut devices: Vec<BlockDevice> = lsblk_devs.iter().map(|lb| {
        let mut dev = BlockDevice::new(lb.name.clone());
        dev.model      = lb.model.clone();
        dev.serial     = lb.serial.clone();
        dev.capacity_bytes = lb.size;
        dev.rotational = lb.rotational;
        dev.transport  = lb.transport.clone();
        dev.partitions = lb.partitions.clone();
        dev.infer_type();

        // Pull live SMART (synchronous, single device — acceptable for --report)
        dev.smart = smart_collector::poll_device(&lb.name);

        dev
    })
    .filter(|d| raw_stats.contains_key(&d.name))
    .collect();

    devices.sort_by(|a, b| a.name.cmp(&b.name));
    (devices, fs_list)
}
