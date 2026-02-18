use crate::alerts::{Alert, Severity};
use crate::collectors::{filesystem, lsblk, smart as smart_collector};
use crate::models::device::BlockDevice;
use crate::models::filesystem::Filesystem;
use crate::models::volume::{RaidArray, ZfsPool};
use crate::util::human::fmt_bytes;

// ── Text report ──────────────────────────────────────────────────────

/// Generate a human-readable health report to a String.
pub fn generate(
    devices:     &[BlockDevice],
    filesystems: &[Filesystem],
    alerts:      &[Alert],
    raids:       &[RaidArray],
    pools:       &[ZfsPool],
) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let mut out = String::new();

    out.push_str("═══════════════════════════════════════════════\n");
    out.push_str(&format!("  DTop Health Report — {}\n", now));
    out.push_str("═══════════════════════════════════════════════\n\n");

    // ── Active alerts ─────────────────────────────────────────────────
    out.push_str(&format!("── Active Alerts ({}) ─────────────────────────\n", alerts.len()));
    if alerts.is_empty() {
        out.push_str("  ● All systems nominal\n");
    } else {
        for a in alerts {
            out.push_str(&format!("  [{}]  {}{}\n", a.severity.label(), a.prefix(), a.message));
        }
    }
    out.push('\n');

    // ── Block devices ─────────────────────────────────────────────────
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
        if let Some(smart) = &dev.smart {
            if let Some(nvme) = &smart.nvme {
                out.push_str(&format!(
                    "  Endurance: {}% used  |  Written: {}  |  POH: {} h\n",
                    nvme.percentage_used, fmt_bytes(nvme.bytes_written()), nvme.power_on_hours,
                ));
            } else if let Some(poh) = smart.power_on_hours {
                out.push_str(&format!("  Power On Hours: {} h\n", poh));
            }
        }
        out.push('\n');
    }

    // ── Filesystems ───────────────────────────────────────────────────
    out.push_str(&format!("── Filesystems ({}) ───────────────────────────\n", filesystems.len()));
    out.push_str(&format!(
        "  {:<20} {:<8} {:>10} {:>10} {:>10} {:>6}\n",
        "Mount", "FS", "Total", "Used", "Avail", "Use%"
    ));
    out.push_str(&format!("  {}\n", "─".repeat(68)));
    for fs in filesystems {
        let eta = fs.days_until_full
            .map(|d| format!("  → full ~{:.0}d", d))
            .unwrap_or_default();
        out.push_str(&format!(
            "  {:<20} {:<8} {:>10} {:>10} {:>10} {:>5.1}%{}\n",
            fs.mount, fs.fs_type,
            fmt_bytes(fs.total_bytes), fmt_bytes(fs.used_bytes),
            fmt_bytes(fs.avail_bytes), fs.use_pct(), eta,
        ));
    }
    out.push('\n');

    // ── Software RAID ─────────────────────────────────────────────────
    if !raids.is_empty() {
        out.push_str(&format!("── Software RAID ({}) ─────────────────────────\n", raids.len()));
        for arr in raids {
            let status = if arr.degraded {
                if arr.rebuild_pct.is_some() { "REBUILDING" } else { "DEGRADED" }
            } else { "healthy" };
            let rebuild = arr.rebuild_pct
                .map(|p| format!("  ({:.1}%)", p))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {:8}  {:6}  {:5}  {}  {}{}\n",
                arr.name, arr.level, status, arr.bitmap,
                fmt_bytes(arr.capacity_bytes), rebuild,
            ));
        }
        out.push('\n');
    }

    // ── ZFS pools ─────────────────────────────────────────────────────
    if !pools.is_empty() {
        out.push_str(&format!("── ZFS Pools ({}) ─────────────────────────────\n", pools.len()));
        out.push_str(&format!(
            "  {:<14} {:<10} {:>10} {:>10} {:>10} {:>6}\n",
            "Pool", "Health", "Size", "Alloc", "Free", "Use%"
        ));
        out.push_str(&format!("  {}\n", "─".repeat(66)));
        for pool in pools {
            out.push_str(&format!(
                "  {:<14} {:<10} {:>10} {:>10} {:>10} {:>5.1}%\n",
                pool.name, pool.health,
                fmt_bytes(pool.size_bytes), fmt_bytes(pool.alloc_bytes),
                fmt_bytes(pool.free_bytes), pool.use_pct(),
            ));
            if !pool.scrub_status.is_empty() {
                out.push_str(&format!("    Scrub: {}\n", pool.scrub_status));
            }
        }
        out.push('\n');
    }

    out.push_str("═══════════════════════════════════════════════\n");
    out
}

// ── HTML report ──────────────────────────────────────────────────────

/// Generate a self-contained HTML health report.
pub fn generate_html(
    devices:     &[BlockDevice],
    filesystems: &[Filesystem],
    alerts:      &[Alert],
    raids:       &[RaidArray],
    pools:       &[ZfsPool],
) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut h = String::new();

    h.push_str(HTML_HEAD);
    h.push_str(&format!("<h1>DTop Health Report</h1>\n"));
    h.push_str(&format!("<p class=\"ts\">Generated: {}</p>\n", esc(&now)));

    // ── Alerts ────────────────────────────────────────────────────────
    h.push_str(&format!("<h2>Active Alerts <span class=\"cnt\">{}</span></h2>\n", alerts.len()));
    if alerts.is_empty() {
        h.push_str("<p class=\"ok\">&#10003; All systems nominal</p>\n");
    } else {
        h.push_str("<table><thead><tr><th>Severity</th><th>Source</th><th>Message</th></tr></thead><tbody>\n");
        for a in alerts {
            let (cls, lbl) = match a.severity {
                Severity::Critical => ("crit", "CRIT"),
                Severity::Warning  => ("warn", "WARN"),
                Severity::Info     => ("ok",   "INFO"),
            };
            h.push_str(&format!(
                "<tr><td><span class=\"badge {}\">{}</span></td><td>{}</td><td>{}</td></tr>\n",
                cls, lbl, esc(&a.prefix()), esc(&a.message)
            ));
        }
        h.push_str("</tbody></table>\n");
    }

    // ── Block devices ─────────────────────────────────────────────────
    h.push_str(&format!("<h2>Block Devices <span class=\"cnt\">{}</span></h2>\n", devices.len()));
    h.push_str("<table><thead><tr><th>Device</th><th>Type</th><th>Model</th><th>Cap</th><th>Temp</th><th>SMART</th><th>Health</th><th>POH</th></tr></thead><tbody>\n");
    for dev in devices {
        use crate::util::health_score::{health_score, score_str};
        let model   = esc(dev.model.as_deref().unwrap_or("—"));
        let cap     = esc(&fmt_bytes(dev.capacity_bytes));
        let temp    = dev.temperature().map(|t| format!("{}°C", t)).unwrap_or_else(|| "—".into());
        let (smart_s, smart_cls) = match &dev.smart {
            Some(s) => {
                use crate::models::smart::SmartStatus;
                let cls = match s.status {
                    SmartStatus::Passed  => "ok",
                    SmartStatus::Warning => "warn",
                    SmartStatus::Failed  => "crit",
                    SmartStatus::Unknown => "dim",
                };
                (s.status.label().trim().to_string(), cls)
            }
            None => ("—".into(), "dim"),
        };
        let hs   = health_score(dev);
        let hs_s = score_str(dev);
        let hs_cls = if hs >= 80 { "ok" } else if hs >= 50 { "warn" } else { "crit" };
        let poh = dev.smart.as_ref().and_then(|s| s.power_on_hours)
            .map(|p| format!("{} h", p)).unwrap_or_else(|| "—".into());
        h.push_str(&format!(
            "<tr><td><b>{}</b></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td class=\"{}\">{}</td><td>{}</td></tr>\n",
            esc(&dev.name), esc(dev.dev_type.label().trim()),
            model, cap, esc(&temp),
            smart_cls, esc(&smart_s),
            hs_cls, esc(&hs_s),
            esc(&poh)
        ));
    }
    h.push_str("</tbody></table>\n");

    // ── Filesystems ───────────────────────────────────────────────────
    h.push_str(&format!("<h2>Filesystems <span class=\"cnt\">{}</span></h2>\n", filesystems.len()));
    h.push_str("<table><thead><tr><th>Mount</th><th>FS</th><th>Total</th><th>Used</th><th>Avail</th><th>Use%</th><th>Est. Full</th></tr></thead><tbody>\n");
    for fs in filesystems {
        let pct = fs.use_pct();
        let pct_cls = if pct >= 95.0 { "crit" } else if pct >= 85.0 { "warn" } else { "ok" };
        let eta = fs.days_until_full
            .map(|d| format!("~{:.0}d", d))
            .unwrap_or_else(|| "—".into());
        let eta_cls = fs.days_until_full
            .map(|d| if d <= 3.0 { "crit" } else if d <= 14.0 { "warn" } else { "ok" })
            .unwrap_or("dim");
        h.push_str(&format!(
            "<tr><td><b>{}</b></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{:.1}%</td><td class=\"{}\">{}</td></tr>\n",
            esc(&fs.mount), esc(&fs.fs_type),
            esc(&fmt_bytes(fs.total_bytes)), esc(&fmt_bytes(fs.used_bytes)),
            esc(&fmt_bytes(fs.avail_bytes)),
            pct_cls, pct,
            eta_cls, esc(&eta),
        ));
    }
    h.push_str("</tbody></table>\n");

    // ── Software RAID ─────────────────────────────────────────────────
    if !raids.is_empty() {
        h.push_str(&format!("<h2>Software RAID <span class=\"cnt\">{}</span></h2>\n", raids.len()));
        h.push_str("<table><thead><tr><th>Array</th><th>Level</th><th>State</th><th>Bitmap</th><th>Capacity</th><th>Rebuild</th></tr></thead><tbody>\n");
        for arr in raids {
            let (state_cls, state_s) = if arr.degraded {
                if arr.rebuild_pct.is_some() { ("warn", "REBUILDING") } else { ("crit", "DEGRADED") }
            } else { ("ok", "healthy") };
            let rebuild = arr.rebuild_pct.map(|p| format!("{:.1}%", p)).unwrap_or_else(|| "—".into());
            h.push_str(&format!(
                "<tr><td><b>{}</b></td><td>{}</td><td class=\"{}\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                esc(&arr.name), esc(&arr.level),
                state_cls, state_s,
                esc(&arr.bitmap), esc(&fmt_bytes(arr.capacity_bytes)),
                esc(&rebuild),
            ));
        }
        h.push_str("</tbody></table>\n");
    }

    // ── ZFS pools ─────────────────────────────────────────────────────
    if !pools.is_empty() {
        h.push_str(&format!("<h2>ZFS Pools <span class=\"cnt\">{}</span></h2>\n", pools.len()));
        h.push_str("<table><thead><tr><th>Pool</th><th>Health</th><th>Size</th><th>Alloc</th><th>Free</th><th>Use%</th><th>Scrub</th></tr></thead><tbody>\n");
        for pool in pools {
            let health_cls = if pool.is_healthy() { "ok" } else { "crit" };
            let pct = pool.use_pct();
            let pct_cls = if pct >= 90.0 { "crit" } else if pct >= 75.0 { "warn" } else { "ok" };
            h.push_str(&format!(
                "<tr><td><b>{}</b></td><td class=\"{}\">{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{:.1}%</td><td>{}</td></tr>\n",
                esc(&pool.name),
                health_cls, esc(&pool.health),
                esc(&fmt_bytes(pool.size_bytes)), esc(&fmt_bytes(pool.alloc_bytes)),
                esc(&fmt_bytes(pool.free_bytes)),
                pct_cls, pct,
                esc(&pool.scrub_status),
            ));
        }
        h.push_str("</tbody></table>\n");
    }

    h.push_str("<footer><p>Generated by <b>dtop</b> — disk health monitor</p></footer>\n");
    h.push_str("</body></html>\n");
    h
}

// ── Markdown report ──────────────────────────────────────────────────

/// Generate a GitHub-flavored Markdown health report.
pub fn generate_markdown(
    devices:     &[BlockDevice],
    filesystems: &[Filesystem],
    alerts:      &[Alert],
    raids:       &[RaidArray],
    pools:       &[ZfsPool],
) -> String {
    use crate::util::health_score::health_score;

    let now      = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let hostname = std::process::Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let mut out = String::new();

    out.push_str("# DTop Health Report\n\n");
    out.push_str(&format!("**Generated:** {}  \n**Host:** {}\n\n", now, hostname));

    // ── Alerts ────────────────────────────────────────────────────────
    out.push_str(&format!("## Active Alerts ({})\n\n", alerts.len()));
    if alerts.is_empty() {
        out.push_str("✓ All systems nominal\n\n");
    } else {
        out.push_str("| Severity | Source | Message |\n");
        out.push_str("|----------|--------|---------|\n");
        for a in alerts {
            let sev = match a.severity {
                Severity::Critical => "🔴 CRIT",
                Severity::Warning  => "🟡 WARN",
                Severity::Info     => "🔵 INFO",
            };
            out.push_str(&format!("| {} | {} | {} |\n",
                sev, me(&a.prefix()), me(&a.message)));
        }
        out.push('\n');
    }

    // ── Block devices ─────────────────────────────────────────────────
    out.push_str(&format!("## Block Devices ({})\n\n", devices.len()));
    out.push_str("| Device | Type | Model | Capacity | Temp | SMART | Health | POH |\n");
    out.push_str("|--------|------|-------|----------|------|-------|--------|-----|\n");
    for dev in devices {
        let model   = me(dev.model.as_deref().unwrap_or("—"));
        let cap     = me(&fmt_bytes(dev.capacity_bytes));
        let temp    = dev.temperature().map(|t| format!("{}°C", t)).unwrap_or_else(|| "—".into());
        let smart_s = dev.smart.as_ref()
            .map(|s| s.status.label().trim().to_string())
            .unwrap_or_else(|| "—".into());
        let hs  = if dev.smart.is_some() { health_score(dev).to_string() } else { "—".into() };
        let poh = dev.smart.as_ref().and_then(|s| s.power_on_hours)
            .map(|h| format!("{} h", h))
            .unwrap_or_else(|| "—".into());
        out.push_str(&format!("| **{}** | {} | {} | {} | {} | {} | {} | {} |\n",
            dev.name, dev.dev_type.label().trim(),
            model, cap, me(&temp), me(&smart_s), hs, me(&poh)));
    }
    out.push('\n');

    // ── Filesystems ───────────────────────────────────────────────────
    out.push_str(&format!("## Filesystems ({})\n\n", filesystems.len()));
    out.push_str("| Mount | FS | Total | Used | Avail | Use% | Est. Full |\n");
    out.push_str("|-------|----|-------|------|-------|------|-----------|\n");
    for fs in filesystems {
        let eta = fs.days_until_full
            .map(|d| format!("~{:.0}d", d))
            .unwrap_or_else(|| "—".into());
        out.push_str(&format!("| **{}** | {} | {} | {} | {} | {:.1}% | {} |\n",
            me(&fs.mount), me(&fs.fs_type),
            me(&fmt_bytes(fs.total_bytes)), me(&fmt_bytes(fs.used_bytes)),
            me(&fmt_bytes(fs.avail_bytes)), fs.use_pct(), me(&eta)));
    }
    out.push('\n');

    // ── Software RAID ─────────────────────────────────────────────────
    if !raids.is_empty() {
        out.push_str(&format!("## Software RAID ({})\n\n", raids.len()));
        out.push_str("| Array | Level | State | Bitmap | Capacity | Rebuild |\n");
        out.push_str("|-------|-------|-------|--------|----------|---------|\n");
        for arr in raids {
            let state = if arr.degraded {
                if arr.rebuild_pct.is_some() { "⚠ REBUILDING" } else { "🔴 DEGRADED" }
            } else { "✓ healthy" };
            let rebuild = arr.rebuild_pct
                .map(|p| format!("{:.1}%", p))
                .unwrap_or_else(|| "—".into());
            out.push_str(&format!("| **{}** | {} | {} | {} | {} | {} |\n",
                arr.name, me(&arr.level), state,
                me(&arr.bitmap), me(&fmt_bytes(arr.capacity_bytes)), me(&rebuild)));
        }
        out.push('\n');
    }

    // ── ZFS pools ─────────────────────────────────────────────────────
    if !pools.is_empty() {
        out.push_str(&format!("## ZFS Pools ({})\n\n", pools.len()));
        out.push_str("| Pool | Health | Size | Alloc | Free | Use% | Scrub |\n");
        out.push_str("|------|--------|------|-------|------|------|-------|\n");
        for pool in pools {
            let health = if pool.is_healthy() { format!("✓ {}", pool.health) } else { format!("🔴 {}", pool.health) };
            out.push_str(&format!("| **{}** | {} | {} | {} | {} | {:.1}% | {} |\n",
                pool.name, health,
                me(&fmt_bytes(pool.size_bytes)), me(&fmt_bytes(pool.alloc_bytes)),
                me(&fmt_bytes(pool.free_bytes)), pool.use_pct(),
                me(&pool.scrub_status)));
        }
        out.push('\n');
    }

    out.push_str("---\n*Generated by **dtop** disk health monitor*\n");
    out
}

/// Escape pipe characters for Markdown table cells.
fn me(s: &str) -> String {
    s.replace('|', "\\|")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

const HTML_HEAD: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>DTop Health Report</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{background:#1e1e2e;color:#cdd6f4;font-family:'Courier New',monospace;font-size:14px;line-height:1.5;padding:24px}
h1{color:#89b4fa;font-size:22px;margin-bottom:4px}
h2{color:#89dceb;font-size:15px;margin:20px 0 8px;padding-bottom:4px;border-bottom:1px solid #313244}
p.ts{color:#6c7086;font-size:12px;margin-bottom:16px}
table{width:100%;border-collapse:collapse;margin-bottom:4px;font-size:13px}
thead tr{background:#313244}
th{padding:6px 10px;text-align:left;color:#89b4fa;font-weight:normal;white-space:nowrap}
td{padding:5px 10px;border-bottom:1px solid #181825;white-space:nowrap}
tr:hover td{background:#252535}
.ok{color:#a6e3a1}.warn{color:#f9e2af}.crit{color:#f38ba8}.dim{color:#585b70}
.cnt{color:#6c7086;font-weight:normal;font-size:13px;margin-left:6px}
.badge{display:inline-block;padding:1px 7px;border-radius:3px;font-size:12px;font-weight:bold}
.badge.ok{background:#a6e3a1;color:#1e1e2e}
.badge.warn{background:#f9e2af;color:#1e1e2e}
.badge.crit{background:#f38ba8;color:#1e1e2e}
footer{margin-top:32px;color:#45475a;font-size:11px}
</style>
</head>
<body>
"#;

// ── Snapshot collector ────────────────────────────────────────────────

/// Collect a one-shot snapshot via lsblk + smartctl and return
/// (devices, filesystems) suitable for report/HTML generation.
pub fn collect_snapshot() -> (Vec<BlockDevice>, Vec<Filesystem>) {
    use crate::collectors::diskstats;

    let lsblk_devs = lsblk::run_lsblk().unwrap_or_default();
    let raw_stats  = diskstats::read_diskstats().unwrap_or_default();
    let fs_list    = filesystem::read_filesystems().unwrap_or_default();

    let mut devices: Vec<BlockDevice> = lsblk_devs.iter().map(|lb| {
        let mut dev = BlockDevice::new(lb.name.clone());
        dev.model          = lb.model.clone();
        dev.serial         = lb.serial.clone();
        dev.capacity_bytes = lb.size;
        dev.rotational     = lb.rotational;
        dev.transport      = lb.transport.clone();
        dev.partitions     = lb.partitions.clone();
        dev.infer_type();
        dev.smart = smart_collector::poll_device(&lb.name);
        dev
    })
    .filter(|d| raw_stats.contains_key(&d.name))
    .collect();

    devices.sort_by(|a, b| a.name.cmp(&b.name));
    (devices, fs_list)
}
