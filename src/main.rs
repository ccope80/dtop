mod alerts;
mod app;
mod collectors;
mod config;
mod input;
mod models;
mod ui;
mod util;

use app::App;
use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::panic;

#[derive(Parser, Debug)]
#[command(name = "dtop", about = "btop-style disk health monitor", version = "0.1")]
struct Cli {
    /// Update interval in milliseconds
    #[arg(short, long, default_value_t = 2000)]
    interval: u64,

    /// Disable SMART data collection
    #[arg(long)]
    no_smart: bool,

    /// Color theme: default, dracula, gruvbox, nord
    #[arg(short = 't', long, default_value = "default")]
    theme: String,

    /// Print a one-shot JSON snapshot of all disk data and exit
    #[arg(long)]
    json: bool,

    /// Print a human-readable health report and exit
    #[arg(long)]
    report: bool,

    /// Run as a headless daemon (no TUI): poll data, evaluate alerts, write log & webhook
    #[arg(long)]
    daemon: bool,

    /// One-shot health check: exit 0=OK, 1=WARNING, 2=CRITICAL (nagios/cron compatible)
    #[arg(long)]
    check: bool,

    /// Print recent alert log entries and exit
    #[arg(long)]
    alerts: bool,

    /// Number of alert log entries to show (used with --alerts)
    #[arg(long, default_value_t = 50)]
    last: usize,

    /// Print config file path and current values, then exit
    #[arg(long)]
    config: bool,

    /// Compare two JSON snapshots (--json output): dtop --diff a.json b.json
    #[arg(long, num_args = 2, value_names = ["FILE_A", "FILE_B"])]
    diff: Option<Vec<String>>,

    /// Print shell completion script and exit (bash, zsh, fish, elvish, powershell)
    #[arg(long, value_name = "SHELL")]
    completions: Option<String>,

    /// Print a one-line health summary and exit (exit 0=OK, 1=WARN, 2=CRIT)
    #[arg(long)]
    summary: bool,

    /// Export current device snapshot as CSV and exit
    #[arg(long)]
    csv: bool,

    /// Print a rolling status snapshot every N seconds (0 = once and exit)
    #[arg(long, value_name = "SECS")]
    watch: Option<u64>,

    /// Open config file in $EDITOR (creates default if missing)
    #[arg(long)]
    edit_config: bool,

    /// Generate a self-contained HTML health report and exit
    #[arg(long)]
    report_html: bool,

    /// Output file path for --report-html (default: dtop-report-TIMESTAMP.html)
    #[arg(long, value_name = "FILE")]
    output: Option<String>,

    /// Only show alerts newer than this duration (e.g. 24h, 7d, 30m) — used with --alerts
    #[arg(long, value_name = "DURATION")]
    since: Option<String>,

    /// Show top processes by disk I/O (2-second sample) and exit
    #[arg(long)]
    top_io: bool,

    /// Number of processes to show with --top-io (default 10)
    #[arg(long, default_value_t = 10)]
    count: usize,

    /// Print a detailed per-device SMART report and exit
    #[arg(long, value_name = "DEVICE")]
    device_report: Option<String>,

    /// Print all tracked SMART anomalies (from persisted log) and exit
    #[arg(long)]
    anomalies: bool,

    /// Print per-device write endurance summary and exit
    #[arg(long)]
    endurance: bool,

    /// List all saved SMART baselines and exit
    #[arg(long)]
    baselines: bool,

    /// Schedule a SMART self-test for DEVICE and exit (short by default)
    #[arg(long, value_name = "DEVICE")]
    schedule_test: Option<String>,

    /// Use a long/extended self-test instead of short (used with --schedule-test)
    #[arg(long)]
    long: bool,

    /// Poll until the self-test completes (used with --schedule-test; Ctrl-C safe)
    #[arg(long)]
    wait: bool,

    /// Poll SMART and save a baseline snapshot for DEVICE, then exit
    #[arg(long, value_name = "DEVICE")]
    save_baseline: Option<String>,

    /// Clear SMART anomaly records and exit; omit DEVICE to clear all
    #[arg(long, value_name = "DEVICE", num_args = 0..=1, default_missing_value = "ALL")]
    clear_anomalies: Option<String>,

    /// Skip confirmation prompts (used with --clear-anomalies)
    #[arg(long)]
    yes: bool,

    /// Print a systemd service unit for dtop --daemon and exit
    #[arg(long)]
    print_service: bool,

    /// Send a test notification to the configured webhook URL and exit
    #[arg(long)]
    test_webhook: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.json {
        return run_json_snapshot();
    }
    if cli.report {
        return run_report();
    }
    if cli.report_html {
        return run_report_html(cli.output.as_deref());
    }
    if cli.check {
        return run_check(!cli.no_smart);
    }
    if cli.alerts {
        return run_alerts(cli.last, cli.since.as_deref());
    }
    if cli.top_io {
        return run_top_io(cli.count);
    }
    if let Some(dev) = &cli.device_report {
        return run_device_report(dev);
    }
    if cli.anomalies {
        return run_anomalies();
    }
    if cli.endurance {
        return run_endurance();
    }
    if cli.baselines {
        return run_baselines();
    }
    if let Some(dev) = &cli.schedule_test {
        return run_schedule_test(dev, cli.long, cli.wait);
    }
    if let Some(dev) = &cli.save_baseline {
        return run_save_baseline(dev);
    }
    if let Some(dev_or_all) = &cli.clear_anomalies {
        let device = if dev_or_all == "ALL" { None } else { Some(dev_or_all.as_str()) };
        return run_clear_anomalies(device, cli.yes);
    }
    if cli.print_service {
        return run_print_service();
    }
    if cli.test_webhook {
        return run_test_webhook();
    }
    if cli.config {
        return run_print_config();
    }
    if let Some(files) = &cli.diff {
        return run_diff(&files[0], &files[1]);
    }
    if let Some(shell) = &cli.completions {
        return run_completions(shell);
    }
    if cli.summary {
        return run_summary(!cli.no_smart);
    }
    if cli.csv {
        return run_csv(!cli.no_smart);
    }
    if let Some(secs) = cli.watch {
        return run_watch(secs, !cli.no_smart);
    }
    if cli.edit_config {
        return run_edit_config();
    }
    if cli.daemon {
        return run_daemon(cli.interval, !cli.no_smart);
    }

    let initial_theme = ui::theme::ThemeVariant::from_name(&cli.theme);

    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        original_hook(info);
    }));

    let result = run(initial_theme, cli.interval, !cli.no_smart);
    restore_terminal()?;
    result
}

fn run_json_snapshot() -> Result<()> {
    use collectors::{filesystem, lsblk, mdraid, nfs, smart_cache, zfs};
    use serde_json::{json, Value};
    use util::human::fmt_bytes;

    let lsblk_devs  = lsblk::run_lsblk().unwrap_or_default();
    let fs_list     = filesystem::read_filesystems().unwrap_or_default();
    let nfs_mounts  = nfs::read_nfs_mounts();
    let smart_cache = smart_cache::load();

    // Build device array
    let devices: Vec<Value> = lsblk_devs.iter().map(|dev| {
        let smart = smart_cache.get(&dev.name).map(|s| {
            json!({
                "status":         s.status.label().trim(),
                "temperature":    s.temperature,
                "power_on_hours": s.power_on_hours,
                "attributes": s.attributes.iter().map(|a| json!({
                    "id":        a.id,
                    "name":      a.name,
                    "value":     a.value,
                    "worst":     a.worst,
                    "thresh":    a.thresh,
                    "raw_value": a.raw_value,
                    "prefail":   a.prefail,
                    "at_risk":   a.is_at_risk(),
                })).collect::<Vec<_>>(),
            })
        });
        let dev_type = if dev.transport.as_deref().unwrap_or("").contains("nvme") {
            "NVMe"
        } else if !dev.rotational {
            "SSD"
        } else if dev.rotational {
            "HDD"
        } else {
            "Unknown"
        };
        json!({
            "name":        dev.name,
            "model":       dev.model,
            "serial":      dev.serial,
            "dev_type":    dev_type,
            "capacity":    dev.size,
            "capacity_hr": fmt_bytes(dev.size),
            "rotational":  dev.rotational,
            "transport":   dev.transport,
            "smart":       smart,
        })
    }).collect();

    // Build filesystem array
    let filesystems: Vec<Value> = fs_list.iter().map(|fs| {
        json!({
            "device":     fs.device,
            "mountpoint": fs.mount,
            "fstype":     fs.fs_type,
            "total":      fs.total_bytes,
            "used":       fs.used_bytes,
            "avail":      fs.avail_bytes,
            "total_hr":   fmt_bytes(fs.total_bytes),
            "used_hr":    fmt_bytes(fs.used_bytes),
            "avail_hr":   fmt_bytes(fs.avail_bytes),
            "use_pct":    fs.use_pct(),
        })
    }).collect();

    // Build NFS array
    let nfs_out: Vec<Value> = nfs_mounts.iter().map(|m| {
        json!({
            "device":           m.device,
            "mount":            m.mount,
            "fstype":           m.fstype,
            "age_secs":         m.age_secs,
            "read_ops":         m.read_ops,
            "write_ops":        m.write_ops,
            "read_rtt_ms":      m.read_rtt_ms,
            "write_rtt_ms":     m.write_rtt_ms,
            "server_bytes_read":    m.server_bytes_read,
            "server_bytes_written": m.server_bytes_written,
        })
    }).collect();

    // RAID arrays
    let raids = mdraid::read_mdstat();
    let raids_out: Vec<Value> = raids.iter().map(|arr| json!({
        "name":           arr.name,
        "state":          arr.state,
        "level":          arr.level,
        "members":        arr.members,
        "capacity":       arr.capacity_bytes,
        "capacity_hr":    fmt_bytes(arr.capacity_bytes),
        "degraded":       arr.degraded,
        "rebuild_pct":    arr.rebuild_pct,
        "bitmap":         arr.bitmap,
    })).collect();

    // ZFS pools
    let pools = zfs::read_zpools();
    let pools_out: Vec<Value> = pools.iter().map(|pool| json!({
        "name":        pool.name,
        "health":      pool.health,
        "size":        pool.size_bytes,
        "size_hr":     fmt_bytes(pool.size_bytes),
        "alloc":       pool.alloc_bytes,
        "alloc_hr":    fmt_bytes(pool.alloc_bytes),
        "free":        pool.free_bytes,
        "free_hr":     fmt_bytes(pool.free_bytes),
        "use_pct":     pool.use_pct(),
        "scrub_status":pool.scrub_status,
    })).collect();

    // PSI (best-effort)
    let psi_out = collectors::pressure::read_pressure().map(|p| json!({
        "io": {
            "some_avg10":  p.io.some.avg10,
            "some_avg60":  p.io.some.avg60,
            "some_avg300": p.io.some.avg300,
            "full_avg10":  p.io.full.avg10,
            "full_avg60":  p.io.full.avg60,
            "full_avg300": p.io.full.avg300,
        },
        "cpu": {
            "some_avg10":  p.cpu.some.avg10,
        },
        "mem": {
            "some_avg10":  p.mem.some.avg10,
        },
    }));

    // SMART anomalies
    let anomaly_log = util::smart_anomaly::load();
    let anomalies_out: serde_json::Map<String, Value> = anomaly_log.iter().map(|(dev, dev_log)| {
        let records: Vec<Value> = dev_log.values().map(|r| json!({
            "attr_id":     r.attr_id,
            "attr_name":   r.attr_name,
            "first_seen":  util::smart_anomaly::fmt_ts(r.first_seen),
            "first_value": r.first_value,
            "last_value":  r.last_value,
            "change":      r.last_value as i64 - r.first_value as i64,
        })).collect();
        (dev.clone(), Value::Array(records))
    }).collect();

    // Write endurance
    let endurance_map = util::write_endurance::load();
    let endurance_out: Vec<Value> = {
        let mut rows: Vec<(&String, &util::write_endurance::DeviceEndurance)> = endurance_map.iter().collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        rows.iter().map(|(dev, e)| {
            let (daily, days) = util::write_endurance::daily_avg(e);
            json!({
                "device":               dev,
                "total_bytes_written":  e.total_bytes_written,
                "total_written_hr":     fmt_bytes(e.total_bytes_written),
                "daily_avg_bytes":      daily as u64,
                "daily_avg_hr":         fmt_bytes(daily as u64),
                "days_tracked":         days,
                "first_tracked_at":     e.first_tracked_at,
            })
        }).collect()
    };

    // Saved baselines
    let baselines_out: Vec<Value> = {
        let base_dir = dirs::data_local_dir().map(|p| p.join("dtop").join("baselines"));
        let mut bls: Vec<Value> = Vec::new();
        if let Some(dir) = base_dir {
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if let Some(bl) = util::smart_baseline::load(stem) {
                                bls.push(json!({
                                    "device":         bl.device,
                                    "saved_date":     bl.saved_date,
                                    "power_on_hours": bl.power_on_hours,
                                    "attribute_count":bl.attributes.len(),
                                }));
                            }
                        }
                    }
                }
            }
        }
        bls.sort_by(|a, b| a["device"].as_str().cmp(&b["device"].as_str()));
        bls
    };

    let snapshot = json!({
        "dtop_version":   "0.1",
        "timestamp":      chrono::Local::now().to_rfc3339(),
        "devices":        devices,
        "filesystems":    filesystems,
        "nfs_mounts":     nfs_out,
        "raid_arrays":    raids_out,
        "zfs_pools":      pools_out,
        "psi":            psi_out,
        "anomalies":      anomalies_out,
        "write_endurance":endurance_out,
        "baselines":      baselines_out,
    });

    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}

fn run_report() -> Result<()> {
    use util::report;
    let cfg = config::Config::load();
    let (devices, filesystems) = report::collect_snapshot();
    let raids = collectors::mdraid::read_mdstat();
    let pools = collectors::zfs::read_zpools();
    let mut all_alerts = alerts::evaluate(&devices, &filesystems, &cfg.alerts);
    all_alerts.extend(alerts::evaluate_volumes(&raids, &pools));
    all_alerts.sort_by(|a, b| b.severity.cmp(&a.severity));
    print!("{}", report::generate(&devices, &filesystems, &all_alerts, &raids, &pools));
    Ok(())
}

fn run_report_html(output: Option<&str>) -> Result<()> {
    use util::report;
    let cfg = config::Config::load();
    let (devices, filesystems) = report::collect_snapshot();
    let raids = collectors::mdraid::read_mdstat();
    let pools = collectors::zfs::read_zpools();
    let mut all_alerts = alerts::evaluate(&devices, &filesystems, &cfg.alerts);
    all_alerts.extend(alerts::evaluate_volumes(&raids, &pools));
    all_alerts.sort_by(|a, b| b.severity.cmp(&a.severity));
    let html = report::generate_html(&devices, &filesystems, &all_alerts, &raids, &pools);

    match output {
        Some(path) => {
            std::fs::write(path, &html)?;
            println!("Report written to: {}", path);
        }
        None => {
            // Auto-name: dtop-report-YYYYMMDD-HHmmss.html in current dir
            let ts   = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let path = format!("dtop-report-{}.html", ts);
            std::fs::write(&path, &html)?;
            println!("Report written to: {}", path);
        }
    }
    Ok(())
}

fn run_print_config() -> Result<()> {
    let cfg = config::Config::load();
    let path = config::Config::config_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown)".to_string());
    let t = &cfg.alerts.thresholds;
    println!("Config: {}", path);
    println!("");
    println!("[general]");
    println!("  update_interval_ms = {}", cfg.general.update_interval_ms);
    println!("  smart_interval_sec = {}", cfg.general.smart_interval_sec);
    println!("");
    println!("[alerts.thresholds]");
    println!("  filesystem_warn_pct   = {}%", t.filesystem_warn_pct);
    println!("  filesystem_crit_pct   = {}%", t.filesystem_crit_pct);
    println!("  inode_warn_pct        = {}%", t.inode_warn_pct);
    println!("  inode_crit_pct        = {}%", t.inode_crit_pct);
    println!("  temperature_warn_ssd  = {}°C", t.temperature_warn_ssd);
    println!("  temperature_crit_ssd  = {}°C", t.temperature_crit_ssd);
    println!("  temperature_warn_hdd  = {}°C", t.temperature_warn_hdd);
    println!("  temperature_crit_hdd  = {}°C", t.temperature_crit_hdd);
    println!("  io_util_warn_pct      = {}%", t.io_util_warn_pct);
    println!("  latency_warn_ms       = {}ms", t.latency_warn_ms);
    println!("  latency_crit_ms       = {}ms", t.latency_crit_ms);
    let fw = if t.fill_days_warn > 0.0 { format!("{:.0}d", t.fill_days_warn) } else { "disabled".into() };
    let fc = if t.fill_days_crit > 0.0 { format!("{:.0}d", t.fill_days_crit) } else { "disabled".into() };
    println!("  fill_days_warn        = {}", fw);
    println!("  fill_days_crit        = {}", fc);
    println!("  cooldown_hours        = {}", cfg.alerts.cooldown_hours);
    println!("");
    if cfg.alerts.smart_rules.is_empty() {
        println!("[alerts.smart_rules]  (none configured — all disabled)");
    } else {
        println!("[alerts.smart_rules]  ({} rules)", cfg.alerts.smart_rules.len());
        for r in &cfg.alerts.smart_rules {
            let msg = r.message.as_deref().unwrap_or("(auto)");
            println!("  attr {:>3}  {} {}  [{}]  {}", r.attr, r.op, r.value, r.severity, msg);
        }
    }
    println!("");
    println!("[devices]");
    println!("  exclude = {:?}", cfg.devices.exclude);
    if cfg.devices.aliases.is_empty() {
        println!("  aliases = (none)");
    } else {
        for (k, v) in &cfg.devices.aliases {
            println!("  alias: {} → {}", k, v);
        }
    }
    println!("");
    println!("[notifications]");
    let webhook = if cfg.notifications.webhook_url.is_empty() { "(not set)" } else { "(configured)" };
    println!("  webhook_url    = {}", webhook);
    println!("  notify_critical = {}", cfg.notifications.notify_critical);
    println!("  notify_warning  = {}", cfg.notifications.notify_warning);
    println!("  notify_send     = {}", cfg.notifications.notify_send);
    Ok(())
}

fn run_alerts(n: usize, since: Option<&str>) -> Result<()> {
    use util::alert_log;
    use alerts::Severity;
    use chrono::NaiveDateTime;

    let entries = if let Some(since_str) = since {
        let duration = parse_since(since_str).ok_or_else(|| {
            anyhow::anyhow!("Invalid --since value '{}'. Use format like 24h, 7d, or 30m.", since_str)
        })?;
        let cutoff = chrono::Local::now().naive_local() - duration;
        let mut all = alert_log::load_all();  // newest-first
        all.retain(|(ts, _)| {
            NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
                .map(|t| t >= cutoff)
                .unwrap_or(false)
        });
        all.reverse();  // oldest-first for display
        if all.is_empty() {
            println!("No alerts in the last {}.", since_str);
            return Ok(());
        }
        all
    } else {
        let entries = alert_log::load_recent(n);
        if entries.is_empty() {
            println!("No alerts in log.");
            return Ok(());
        }
        entries
    };

    for (ts, alert) in &entries {
        let sev = match alert.severity {
            Severity::Critical => "CRIT",
            Severity::Warning  => "WARN",
            Severity::Info     => "INFO",
        };
        println!("{} [{}] {}", ts, sev, alert.message);
    }
    Ok(())
}

fn parse_since(s: &str) -> Option<chrono::Duration> {
    let s = s.trim().to_lowercase();
    if let Some(n) = s.strip_suffix('h') {
        return Some(chrono::Duration::hours(n.trim().parse::<i64>().ok()?));
    }
    if let Some(n) = s.strip_suffix('d') {
        return Some(chrono::Duration::days(n.trim().parse::<i64>().ok()?));
    }
    if let Some(n) = s.strip_suffix('m') {
        return Some(chrono::Duration::minutes(n.trim().parse::<i64>().ok()?));
    }
    None
}

fn run_print_service() -> Result<()> {
    let exe = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("/usr/local/bin/dtop"));
    let exe_str = exe.to_string_lossy();

    println!("[Unit]");
    println!("Description=DTop Disk Health Monitor Daemon");
    println!("Documentation=https://github.com/ccope80/dtop");
    println!("After=multi-user.target");
    println!();
    println!("[Service]");
    println!("Type=simple");
    println!("ExecStart={} --daemon", exe_str);
    println!("Restart=always");
    println!("RestartSec=30");
    println!("User=root");
    println!("StandardOutput=journal");
    println!("StandardError=journal");
    println!("SyslogIdentifier=dtop");
    println!();
    println!("[Install]");
    println!("WantedBy=multi-user.target");
    println!();
    println!("# Install:");
    println!("#   dtop --print-service | sudo tee /etc/systemd/system/dtop.service");
    println!("#   sudo systemctl daemon-reload");
    println!("#   sudo systemctl enable --now dtop");
    println!("#   journalctl -u dtop -f");
    Ok(())
}

fn run_test_webhook() -> Result<()> {
    let cfg = config::Config::load();
    if cfg.notifications.webhook_url.is_empty() {
        eprintln!(
            "No webhook URL configured.\n\
             Set notifications.webhook_url in ~/.config/dtop/dtop.toml.\n\
             Use 'dtop --edit-config' to open the config file."
        );
        std::process::exit(1);
    }

    let url = &cfg.notifications.webhook_url;
    println!("Sending test notification to webhook…");
    println!("URL: {}", url);

    let hostname = std::process::Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let payload = format!(
        "{{\"text\":\"[dtop] Test notification from {} — webhook integration is working correctly.\"}}",
        hostname
    );

    let out = std::process::Command::new("curl")
        .args([
            "-s", "-i", "--max-time", "10",
            "-X", "POST",
            "-H", "Content-Type: application/json",
            "-d", &payload,
            url,
        ])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run curl: {}\nIs curl installed?", e))?;

    let response = String::from_utf8_lossy(&out.stdout);
    let status_line = response.lines().next().unwrap_or("(no response)");
    println!("Response: {}", status_line.trim());

    // HTTP 2xx = success
    let ok = status_line.contains(" 2");
    if ok {
        println!("✓ Webhook delivered successfully.");
    } else {
        eprintln!("✗ Webhook delivery may have failed.");
        let body: String = response.lines()
            .skip_while(|l| !l.is_empty())
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n");
        if !body.trim().is_empty() {
            eprintln!("Body: {}", body.trim());
        }
        std::process::exit(1);
    }
    Ok(())
}

/// One entry from a SMART self-test log (ATA or NVMe).
struct SelfTestEntry {
    test_type: String,
    status:    String,
    hours:     u64,
    passed:    bool,
}

/// Run `smartctl --json=c -a /dev/{name}` and parse the self-test log table.
/// Returns ATA or NVMe entries (whichever the drive reports), newest first.
fn fetch_selftest_log(name: &str) -> Vec<SelfTestEntry> {
    use serde_json::Value;

    let out = match std::process::Command::new("smartctl")
        .args(["--json=c", "-a", &format!("/dev/{}", name)])
        .output()
    {
        Ok(o)  => o,
        Err(_) => return vec![],
    };

    let v: Value = match serde_json::from_slice(&out.stdout) {
        Ok(v)  => v,
        Err(_) => return vec![],
    };

    let mut entries: Vec<SelfTestEntry> = Vec::new();

    // ATA drives
    if let Some(table) = v["ata_smart_self_test_log"]["standard"]["table"].as_array() {
        for row in table {
            entries.push(SelfTestEntry {
                test_type: row["type"]["string"].as_str().unwrap_or("?").to_string(),
                status:    row["status"]["string"].as_str().unwrap_or("?").to_string(),
                hours:     row["lifetime_hours"].as_u64().unwrap_or(0),
                passed:    row["status"]["passed"].as_bool().unwrap_or(false),
            });
        }
    }

    // NVMe drives
    if let Some(table) = v["nvme_self_test_log"]["table"].as_array() {
        for row in table {
            entries.push(SelfTestEntry {
                test_type: row["self_test_code"]["string"].as_str().unwrap_or("?").to_string(),
                status:    row["self_test_result"]["string"].as_str().unwrap_or("?").to_string(),
                hours:     row["power_on_hours"].as_u64().unwrap_or(0),
                passed:    row["self_test_result"]["value"].as_u64() == Some(0),
            });
        }
    }

    entries
}

fn run_schedule_test(device: &str, long_test: bool, wait: bool) -> Result<()> {
    let name      = device.trim_start_matches("/dev/");
    let dev_path  = format!("/dev/{}", name);
    let test_type = if long_test { "long" } else { "short" };
    let eta       = if long_test { "(may take hours on large HDDs)" } else { "(~2 minutes)" };

    println!("Scheduling {} SMART self-test on {} {}…", test_type, dev_path, eta);

    let out = std::process::Command::new("smartctl")
        .args(["-t", test_type, &dev_path])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run smartctl: {}\nIs smartctl installed?", e))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.contains("previous self-test") || stdout.contains("test already in progress") {
        println!("A self-test is already running on {} — try again after it completes.", dev_path);
    } else if stdout.contains("Test has begun") || stdout.contains("SMART offline immediate test") {
        println!("Test scheduled successfully.");
    } else if !out.status.success() {
        eprintln!("smartctl exited {}: {}", out.status, stdout.trim());
        std::process::exit(1);
    } else {
        // Unknown but non-error output — print it and continue
        println!("{}", stdout.trim());
    }

    if !wait {
        println!("Tip: re-run with --wait to block until completion, or use --device-report {} to check later.", name);
        return Ok(());
    }

    let poll_secs = if long_test { 120u64 } else { 30u64 };
    println!("Polling every {}s (Ctrl-C is safe — the test continues on-device)…", poll_secs);

    loop {
        std::thread::sleep(std::time::Duration::from_secs(poll_secs));

        let poll = match std::process::Command::new("smartctl")
            .args(["-a", &dev_path])
            .output()
        {
            Ok(o)  => o,
            Err(e) => { eprintln!("Poll error: {}", e); continue; }
        };

        let text = String::from_utf8_lossy(&poll.stdout);

        if let Some(remaining) = cli_parse_smart_test_remaining(&text) {
            let done = 100u8.saturating_sub(remaining);
            let now  = chrono::Local::now().format("%H:%M:%S");
            println!("  [{}]  {}% complete  ({}% remaining)", now, done, remaining);
        } else if text.contains("without error") {
            println!("✓  Self-test completed successfully.");
            break;
        } else if text.contains("FAILED!") || (text.contains("# 1") && text.contains("Failed")) {
            eprintln!("✗  Self-test FAILED — run 'dtop --device-report {}' for details.", name);
            std::process::exit(2);
        } else if text.contains("borted") {
            println!("⚠  Self-test was aborted.");
            break;
        }
        // else: result is ambiguous (test may not have started yet) — keep polling
    }
    Ok(())
}

/// Extract the "X% of test remaining" value from smartctl -a output.
fn cli_parse_smart_test_remaining(text: &str) -> Option<u8> {
    for line in text.lines() {
        if line.contains("% of test remaining") {
            for word in line.split_whitespace() {
                if word.ends_with('%') {
                    return word.trim_end_matches('%').parse::<u8>().ok();
                }
            }
        }
    }
    None
}

fn run_save_baseline(device: &str) -> Result<()> {
    use collectors::smart as smart_collector;
    use util::smart_baseline;

    let name = device.trim_start_matches("/dev/");
    println!("Polling SMART data for /dev/{}…", name);

    let smart = match smart_collector::poll_device(name) {
        Some(s) => s,
        None => {
            eprintln!(
                "SMART data unavailable for /dev/{}.\n\
                 Is smartctl installed and does the device support SMART?",
                name
            );
            std::process::exit(1);
        }
    };

    smart_baseline::save(name, &smart);

    println!("Baseline saved for /dev/{}", name);
    if let Some(h) = smart.power_on_hours {
        println!("  Power-On Hours : {} h  ({:.1} yr)", h, h as f64 / 8760.0);
    }
    println!("  SMART Status   : {}", smart.status.label().trim());
    println!("  Attributes     : {}", smart.attributes.len());
    println!("  Date           : {}", chrono::Local::now().format("%Y-%m-%d"));
    println!("\nUse 'dtop --device-report {}' to compare current SMART against this baseline.", name);
    Ok(())
}

fn run_clear_anomalies(device: Option<&str>, yes: bool) -> Result<()> {
    use util::smart_anomaly;
    use std::io::Write;

    let mut log = smart_anomaly::load();
    if log.is_empty() {
        println!("No anomalies tracked — nothing to clear.");
        return Ok(());
    }

    let (desc, count) = match device {
        None => {
            let n: usize = log.values().map(|d| d.len()).sum();
            (format!("all {} device(s)", log.len()), n)
        }
        Some(dev) => {
            let n = log.get(dev).map(|d| d.len()).unwrap_or(0);
            if n == 0 {
                println!("No anomalies tracked for '{}'.", dev);
                return Ok(());
            }
            (format!("device '{}'", dev), n)
        }
    };

    if !yes {
        print!("Clear {} anomal{} from {}? [y/N] ",
            count, if count == 1 { "y" } else { "ies" }, desc);
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    match device {
        None      => log.clear(),
        Some(dev) => { log.remove(dev); }
    }

    smart_anomaly::save(&log);
    println!("Cleared {} anomal{} from {}.",
        count, if count == 1 { "y" } else { "ies" }, desc);
    Ok(())
}

fn run_anomalies() -> Result<()> {
    use util::smart_anomaly;

    let log = smart_anomaly::load();
    if log.is_empty() {
        println!("No SMART anomalies tracked yet (anomalies are detected while dtop is running).");
        return Ok(());
    }

    // Flatten and sort: device asc, then attr_id asc
    let mut rows: Vec<(String, &smart_anomaly::AnomalyRecord)> = log
        .iter()
        .flat_map(|(dev, dev_log)| dev_log.values().map(move |r| (dev.clone(), r)))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.attr_id.cmp(&b.1.attr_id)));

    let total = rows.len();
    let devs  = log.len();
    println!("SMART ANOMALY LOG  ({} device{}, {} anomal{})",
        devs, if devs == 1 { "" } else { "s" },
        total, if total == 1 { "y" } else { "ies" });
    println!("{}", "─".repeat(74));
    println!("{:<10}  {:>4}  {:<30}  {:>10}  {:>8}  {:>8}  {:>7}",
        "Device", "ID", "Attribute", "First Seen", "First", "Current", "Change");
    println!("{}", "─".repeat(74));

    for (dev, rec) in &rows {
        let first_date = smart_anomaly::fmt_ts(rec.first_seen);
        let change     = rec.last_value as i64 - rec.first_value as i64;
        let change_str = if change == 0 {
            "     0".to_string()
        } else {
            format!("  {:+}", change)
        };
        let attr_label = if rec.attr_id == 9999 {
            rec.attr_name.clone()
        } else {
            format!("{} ({})", rec.attr_id, rec.attr_name)
        };
        println!("{:<10}  {:>4}  {:<30}  {:>10}  {:>8}  {:>8}  {}",
            dev, rec.attr_id, &attr_label[..attr_label.len().min(30)],
            first_date, rec.first_value, rec.last_value, change_str);
    }
    Ok(())
}

fn run_endurance() -> Result<()> {
    use util::{write_endurance, human::fmt_bytes};

    let map = write_endurance::load();
    if map.is_empty() {
        println!("No write endurance data yet (dtop accumulates this while running).");
        return Ok(());
    }

    let mut rows: Vec<(&String, &write_endurance::DeviceEndurance)> = map.iter().collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));

    println!("WRITE ENDURANCE  ({} device{})", rows.len(), if rows.len() == 1 { "" } else { "s" });
    println!("{}", "─".repeat(70));
    println!("{:<10}  {:>14}  {:>12}  {:>12}  {:>10}",
        "Device", "Total Written", "Daily Avg", "Days Tracked", "Since");
    println!("{}", "─".repeat(70));

    for (dev, e) in &rows {
        let (daily, days) = write_endurance::daily_avg(e);
        let started = {
            use chrono::{Local, TimeZone};
            Local.timestamp_opt(e.first_tracked_at, 0)
                .single()
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        };
        println!("{:<10}  {:>14}  {:>12}  {:>12.1}  {:>10}",
            dev,
            fmt_bytes(e.total_bytes_written),
            fmt_bytes(daily as u64) + "/d",
            days,
            started);
    }
    Ok(())
}

fn run_baselines() -> Result<()> {
    use util::smart_baseline;

    let base_dir = dirs::data_local_dir()
        .map(|p| p.join("dtop").join("baselines"));

    let dir = match base_dir {
        Some(d) if d.exists() => d,
        _ => {
            println!("No baselines saved yet. Open a device in dtop and press B to save one.");
            return Ok(());
        }
    };

    let mut baselines: Vec<smart_baseline::Baseline> = Vec::new();
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if let Some(bl) = smart_baseline::load(stem) {
                    baselines.push(bl);
                }
            }
        }
    }

    if baselines.is_empty() {
        println!("No baselines saved yet. Open a device in dtop and press B to save one.");
        return Ok(());
    }

    baselines.sort_by(|a, b| a.device.cmp(&b.device));

    println!("SMART BASELINES  ({} saved)", baselines.len());
    println!("{}", "─".repeat(60));
    println!("{:<10}  {:>12}  {:>14}  {:>10}",
        "Device", "Saved", "Power-On Hrs", "Attributes");
    println!("{}", "─".repeat(60));
    for bl in &baselines {
        let poh = bl.power_on_hours
            .map(|h| format!("{}", h))
            .unwrap_or_else(|| "—".to_string());
        println!("{:<10}  {:>12}  {:>14}  {:>10}",
            bl.device, bl.saved_date, poh, bl.attributes.len());
    }
    println!("\nUse --device-report DEVICE to compare current SMART against a baseline.");
    Ok(())
}

fn run_top_io(count: usize) -> Result<()> {
    use collectors::process_io;
    use std::collections::HashMap;
    use util::human::fmt_rate;

    eprintln!("Sampling I/O for 2 seconds…");
    let snap1 = process_io::read_all();
    std::thread::sleep(std::time::Duration::from_secs(2));
    let snap2 = process_io::read_all();

    let mut uid_cache: HashMap<u32, String> = HashMap::new();
    let mut rates = process_io::compute_rates(&snap1, &snap2, 2.0, &mut uid_cache);
    rates.sort_by(|a, b| {
        b.total_per_sec().partial_cmp(&a.total_per_sec()).unwrap_or(std::cmp::Ordering::Equal)
    });

    if rates.is_empty() {
        println!("No process I/O detected in the sampling window.");
        return Ok(());
    }

    let n = count.min(rates.len());
    println!("{:>7}  {:<16}  {:<12}  {:>10}  {:>10}  {:>10}",
        "PID", "COMMAND", "USER", "READ/s", "WRITE/s", "TOTAL/s");
    println!("{}", "─".repeat(73));
    for r in &rates[..n] {
        let comm = r.comm.chars().take(16).collect::<String>();
        let user = r.username.chars().take(12).collect::<String>();
        println!("{:>7}  {:<16}  {:<12}  {:>10}  {:>10}  {:>10}",
            r.pid, comm, user,
            fmt_rate(r.read_per_sec), fmt_rate(r.write_per_sec), fmt_rate(r.total_per_sec()));
    }
    Ok(())
}

fn run_device_report(device: &str) -> Result<()> {
    use collectors::{lsblk, smart as smart_collector};
    use models::device::BlockDevice;
    use util::{health_score, human::fmt_bytes, smart_attr_desc};

    let name = device.trim_start_matches("/dev/");
    let devs = lsblk::run_lsblk().unwrap_or_default();
    let lsblk_dev = devs.iter().find(|d| d.name == name);

    let lsblk_dev = match lsblk_dev {
        Some(d) => d,
        None => {
            eprintln!("Device '{}' not found. Available devices:", name);
            for d in &devs { eprintln!("  /dev/{}", d.name); }
            std::process::exit(1);
        }
    };

    let mut dev = BlockDevice::new(lsblk_dev.name.clone());
    dev.model          = lsblk_dev.model.clone();
    dev.serial         = lsblk_dev.serial.clone();
    dev.capacity_bytes = lsblk_dev.size;
    dev.rotational     = lsblk_dev.rotational;
    dev.transport      = lsblk_dev.transport.clone();
    dev.partitions     = lsblk_dev.partitions.clone();
    dev.infer_type();

    eprintln!("Polling SMART data for /dev/{}…", name);
    dev.smart = smart_collector::poll_device(name);

    let bar = "═".repeat(72);
    println!("{}", bar);
    println!("  DTop Device Report — /dev/{}", name);
    println!("{}", bar);

    println!("\nIDENTITY");
    println!("  Name       : /dev/{}", name);
    if let Some(m) = &dev.model  { println!("  Model      : {}", m); }
    if let Some(s) = &dev.serial { println!("  Serial     : {}", s); }
    println!("  Type       : {}", dev.dev_type.label().trim());
    println!("  Capacity   : {}", fmt_bytes(dev.capacity_bytes));
    if let Some(t) = &dev.transport { println!("  Transport  : {}", t); }
    if !dev.partitions.is_empty() {
        let parts: Vec<String> = dev.partitions.iter().map(|p| p.name.clone()).collect();
        println!("  Partitions : {}", parts.join(", "));
    }

    match &dev.smart {
        None => {
            println!("\nSMART data unavailable (device may not support SMART, or smartctl not installed).");
        }
        Some(smart) => {
            let score = health_score::health_score(&dev);
            println!("\nHEALTH SUMMARY");
            println!("  Score      : {} / 100", score);
            println!("  Status     : {}", smart.status.label().trim());
            if let Some(t) = smart.temperature {
                let crit = if dev.rotational { t >= 60 } else { t >= 70 };
                let warn = if dev.rotational { t >= 50 } else { t >= 55 };
                let flag = if crit { "  ← CRITICAL" } else if warn { "  ← WARNING" } else { "" };
                println!("  Temperature: {}°C{}", t, flag);
            }
            if let Some(h) = smart.power_on_hours {
                println!("  Power-On   : {} h  ({:.1} yr)", h, h as f64 / 8760.0);
            }

            // Score breakdown
            println!("\nSCORE BREAKDOWN");
            let mut total_ded: i32 = 0;
            if smart.status == crate::models::smart::SmartStatus::Warning {
                println!("  -10  SMART status Warning");
                total_ded += 10;
            }
            if let Some(t) = smart.temperature {
                let ded: i32 = if dev.rotational {
                    if t >= 60 { 20 } else if t >= 50 { 10 } else { 0 }
                } else {
                    if t >= 70 { 20 } else if t >= 55 { 10 } else { 0 }
                };
                if ded > 0 { println!("  -{:2}  Temperature {}°C", ded, t); total_ded += ded; }
            }
            for attr in &smart.attributes {
                let ded: i32 = match attr.id {
                    5   => if attr.raw_value > 100 { 30 } else if attr.raw_value > 0 { 15 } else { 0 },
                    197 => if attr.raw_value > 0 { 25 } else { 0 },
                    198 => if attr.raw_value > 0 { 40 } else { 0 },
                    _   => 0,
                };
                if ded > 0 {
                    println!("  -{:2}  Attr {:>3} ({}) raw={}", ded, attr.id, attr.name, attr.raw_value);
                    total_ded += ded;
                }
            }
            if let Some(nvme) = &smart.nvme {
                let ded: i32 = match nvme.percentage_used {
                    90..=u8::MAX => 30, 70..=89 => 15, 50..=69 => 5, _ => 0,
                };
                if ded > 0 { println!("  -{:2}  NVMe wear {}% used", ded, nvme.percentage_used); total_ded += ded; }
                if nvme.media_errors > 0 { println!("  -25  NVMe media errors: {}", nvme.media_errors); total_ded += 25; }
                if nvme.available_spare_pct < nvme.available_spare_threshold {
                    println!("  -20  NVMe spare below threshold ({}% < {}%)",
                        nvme.available_spare_pct, nvme.available_spare_threshold);
                    total_ded += 20;
                }
            }
            if total_ded == 0 {
                println!("  (no deductions — healthy)");
            } else {
                println!("  ────  Final score: {} (100 − {})", score, total_ded);
            }

            // ATA SMART attributes table
            if !smart.attributes.is_empty() {
                println!("\nATA SMART ATTRIBUTES");
                println!("  {:>3}  {:<34}  {:>5}/{:>5}/{:>5}  {:<14}  {}",
                    "ID", "Name", "Val", "Wst", "Thr", "Raw", "Flags");
                println!("  {}", "─".repeat(82));
                for attr in &smart.attributes {
                    let flags = format!("{}{}",
                        if attr.prefail { "P" } else { "-" },
                        if attr.is_at_risk() { " RISK" } else { "" });
                    println!("  {:>3}  {:<34}  {:>5}/{:>5}/{:>5}  {:<14}  {}",
                        attr.id, attr.name,
                        attr.value, attr.worst, attr.thresh,
                        attr.raw_str, flags);
                    if let Some(desc) = smart_attr_desc::describe(attr.id) {
                        println!("       ↳ {}", desc);
                    }
                }
            }

            // NVMe health log
            if let Some(nvme) = &smart.nvme {
                println!("\nNVMe HEALTH LOG");
                let cw_flag = if nvme.critical_warning != 0 { "  ← WARNING" } else { "" };
                println!("  Critical Warning  : 0x{:02X}{}", nvme.critical_warning, cw_flag);
                println!("  Temperature       : {}°C", nvme.temperature_celsius);
                let spare_flag = if nvme.available_spare_pct < nvme.available_spare_threshold {
                    "  ← below threshold!"
                } else { "" };
                println!("  Available Spare   : {}%  (threshold: {}%){}",
                    nvme.available_spare_pct, nvme.available_spare_threshold, spare_flag);
                println!("  Percentage Used   : {}%", nvme.percentage_used);
                println!("  Data Read         : {}", fmt_bytes(nvme.bytes_read()));
                println!("  Data Written      : {}", fmt_bytes(nvme.bytes_written()));
                println!("  Power-On Hours    : {}", nvme.power_on_hours);
                println!("  Unsafe Shutdowns  : {}", nvme.unsafe_shutdowns);
                let me_flag = if nvme.media_errors > 0 { "  ← WARNING" } else { "" };
                println!("  Media Errors      : {}{}", nvme.media_errors, me_flag);
                println!("  Error Log Entries : {}", nvme.error_log_entries);

                // Wear projection
                if nvme.power_on_hours > 24 && nvme.percentage_used > 0 {
                    let days_active = nvme.power_on_hours as f64 / 24.0;
                    let daily_rate  = nvme.percentage_used as f64 / days_active;
                    let remain_pct  = 100u64.saturating_sub(nvme.percentage_used as u64) as f64;
                    if daily_rate > 0.0 {
                        let days_left  = remain_pct / daily_rate;
                        let years_left = days_left / 365.25;
                        println!("\nNVMe WEAR PROJECTION");
                        println!("  Wear Rate         : {:.5}%/day", daily_rate);
                        println!("  Estimated Life    : ~{:.0} days  ({:.1} yr remaining)",
                            days_left, years_left);
                    }
                }
            }
        }
    }

    // Self-test log (second smartctl call, best-effort)
    let tests = fetch_selftest_log(name);
    if !tests.is_empty() {
        println!("\nSELF-TEST LOG  ({} entr{})", tests.len(), if tests.len() == 1 { "y" } else { "ies" });
        println!("  {:<2}  {:>6}  {:<22}  {}",
            "", "Hours", "Result", "Test Type");
        println!("  {}", "─".repeat(58));
        for t in &tests {
            let mark = if t.passed { "✓" } else { "✗" };
            // Truncate long status strings for alignment
            let status = if t.status.len() > 22 { &t.status[..22] } else { &t.status };
            println!("  {}   {:>6}  {:<22}  {}",
                mark, t.hours, status, t.test_type);
        }
    }

    println!();
    Ok(())
}

fn run_check(smart_enabled: bool) -> Result<()> {
    use collectors::{filesystem, smart as smart_collector};
    use models::device::BlockDevice;
    use alerts::Severity;

    let cfg = config::Config::load();
    let lsblk_devs = collectors::lsblk::run_lsblk().unwrap_or_default();
    let raw_stats  = collectors::diskstats::read_diskstats().unwrap_or_default();
    let fs_list    = filesystem::read_filesystems().unwrap_or_default();

    let devices: Vec<BlockDevice> = lsblk_devs.iter()
        .filter(|lb| !cfg.devices.exclude.iter().any(|pat| {
            if let Some(p) = pat.strip_suffix('*') { lb.name.starts_with(p) }
            else { pat == &lb.name }
        }))
        .filter(|lb| raw_stats.contains_key(&lb.name))
        .map(|lb| {
            let mut dev = BlockDevice::new(lb.name.clone());
            dev.model = lb.model.clone(); dev.serial = lb.serial.clone();
            dev.capacity_bytes = lb.size; dev.rotational = lb.rotational;
            dev.transport = lb.transport.clone(); dev.partitions = lb.partitions.clone();
            dev.infer_type();
            if smart_enabled { dev.smart = smart_collector::poll_device(&lb.name); }
            dev
        })
        .collect();

    let raids = collectors::mdraid::read_mdstat();
    let pools = collectors::zfs::read_zpools();
    let mut active_alerts = alerts::evaluate(&devices, &fs_list, &cfg.alerts);
    active_alerts.extend(alerts::evaluate_volumes(&raids, &pools));
    active_alerts.sort_by(|a, b| b.severity.cmp(&a.severity));

    let has_crit = active_alerts.iter().any(|a| a.severity == Severity::Critical);
    let has_warn = active_alerts.iter().any(|a| a.severity == Severity::Warning);

    if active_alerts.is_empty() {
        println!("OK — {} device(s), {} filesystem(s), {} array(s), no alerts",
            devices.len(), fs_list.len(), raids.len() + pools.len());
        std::process::exit(0);
    }

    for a in &active_alerts {
        println!("[{}] {}{}", a.severity.label(), a.prefix(), a.message);
    }

    if has_crit {
        std::process::exit(2);
    } else if has_warn {
        std::process::exit(1);
    }
    Ok(())
}

fn run_daemon(interval_ms: u64, smart_enabled: bool) -> Result<()> {
    use collectors::{filesystem, smart as smart_collector};
    use models::device::BlockDevice;
    use util::{alert_log, webhook};

    eprintln!("dtop daemon starting (interval {}ms, SMART {})…",
        interval_ms, if smart_enabled { "enabled" } else { "disabled" });

    let cfg = config::Config::load();
    let mut prev_alerts: Vec<alerts::Alert> = Vec::new();
    let tick = std::time::Duration::from_millis(interval_ms.max(500));

    loop {
        let lsblk_devs = collectors::lsblk::run_lsblk().unwrap_or_default();
        let raw_stats  = collectors::diskstats::read_diskstats().unwrap_or_default();
        let fs_list    = filesystem::read_filesystems().unwrap_or_default();

        let devices: Vec<BlockDevice> = lsblk_devs.iter()
            .filter(|lb| !cfg.devices.exclude.iter().any(|pat| {
                if let Some(p) = pat.strip_suffix('*') { lb.name.starts_with(p) }
                else { pat == &lb.name }
            }))
            .filter(|lb| raw_stats.contains_key(&lb.name))
            .map(|lb| {
                let mut dev = BlockDevice::new(lb.name.clone());
                dev.model = lb.model.clone(); dev.serial = lb.serial.clone();
                dev.capacity_bytes = lb.size; dev.rotational = lb.rotational;
                dev.transport = lb.transport.clone(); dev.partitions = lb.partitions.clone();
                dev.infer_type();
                if smart_enabled { dev.smart = smart_collector::poll_device(&lb.name); }
                dev
            })
            .collect();

        let raids = collectors::mdraid::read_mdstat();
        let pools = collectors::zfs::read_zpools();
        let mut new_alerts = alerts::evaluate(&devices, &fs_list, &cfg.alerts);
        new_alerts.extend(alerts::evaluate_volumes(&raids, &pools));
        new_alerts.sort_by(|a, b| b.severity.cmp(&a.severity));
        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        let mut fresh: Vec<alerts::Alert> = Vec::new();
        for alert in &new_alerts {
            let key = format!("{}{}{}", alert.severity.label(), alert.prefix(), alert.message);
            if !prev_alerts.iter().any(|a| {
                format!("{}{}{}", a.severity.label(), a.prefix(), a.message) == key
            }) {
                fresh.push(alert.clone());
            }
        }
        if !fresh.is_empty() {
            alert_log::append(&fresh);
            if !cfg.notifications.webhook_url.is_empty() {
                webhook::notify(&fresh, &cfg.notifications.webhook_url, cfg.notifications.notify_warning);
            }
            for a in &fresh {
                eprintln!("{} [{}] {}{}", now, a.severity.label(), a.prefix(), a.message);
            }
        }
        prev_alerts = new_alerts;
        std::thread::sleep(tick);
    }
}

fn run_summary(smart_enabled: bool) -> Result<()> {
    use collectors::{filesystem, smart as smart_collector};
    use models::device::BlockDevice;
    use alerts::Severity;

    let cfg = config::Config::load();
    let lsblk_devs = collectors::lsblk::run_lsblk().unwrap_or_default();
    let raw_stats  = collectors::diskstats::read_diskstats().unwrap_or_default();
    let fs_list    = filesystem::read_filesystems().unwrap_or_default();

    let devices: Vec<BlockDevice> = lsblk_devs.iter()
        .filter(|lb| !cfg.devices.exclude.iter().any(|pat| {
            if let Some(p) = pat.strip_suffix('*') { lb.name.starts_with(p) }
            else { pat == &lb.name }
        }))
        .filter(|lb| raw_stats.contains_key(&lb.name))
        .map(|lb| {
            let mut dev = BlockDevice::new(lb.name.clone());
            dev.model = lb.model.clone(); dev.serial = lb.serial.clone();
            dev.capacity_bytes = lb.size; dev.rotational = lb.rotational;
            dev.transport = lb.transport.clone(); dev.partitions = lb.partitions.clone();
            dev.infer_type();
            if smart_enabled { dev.smart = smart_collector::poll_device(&lb.name); }
            dev
        })
        .collect();

    let raids = collectors::mdraid::read_mdstat();
    let pools = collectors::zfs::read_zpools();
    let mut active = alerts::evaluate(&devices, &fs_list, &cfg.alerts);
    active.extend(alerts::evaluate_volumes(&raids, &pools));
    active.sort_by(|a, b| b.severity.cmp(&a.severity));

    let crit_n = active.iter().filter(|a| a.severity == Severity::Critical).count();
    let warn_n = active.iter().filter(|a| a.severity == Severity::Warning).count();

    let status = if crit_n > 0 { "CRIT" } else if warn_n > 0 { "WARN" } else { "OK" };
    println!(
        "{} | devs:{} fs:{} arrays:{} | crit:{} warn:{}",
        status, devices.len(), fs_list.len(), raids.len() + pools.len(), crit_n, warn_n
    );

    if crit_n > 0 {
        std::process::exit(2);
    } else if warn_n > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn run_edit_config() -> Result<()> {
    let path = match config::Config::config_path() {
        Some(p) => p,
        None => {
            eprintln!("Cannot determine config directory.");
            std::process::exit(1);
        }
    };
    // Bootstrap default config if none exists yet
    if !path.exists() {
        config::Config::load(); // triggers try_write_defaults() internally
        println!("Created default config: {}", path.display());
    }
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    println!("Opening {} with {}…", path.display(), editor);
    let status = std::process::Command::new(&editor).arg(&path).status()?;
    if !status.success() {
        eprintln!("{} exited non-zero", editor);
    }
    Ok(())
}

fn run_watch(interval_secs: u64, smart_enabled: bool) -> Result<()> {
    use collectors::{filesystem, smart as smart_collector};
    use models::device::BlockDevice;
    use util::human::{fmt_bytes, fmt_rate};
    use util::health_score::health_score;

    let cfg = config::Config::load();
    let tick = if interval_secs == 0 { None } else { Some(std::time::Duration::from_secs(interval_secs)) };

    loop {
        let lsblk_devs = collectors::lsblk::run_lsblk().unwrap_or_default();
        let raw_stats  = collectors::diskstats::read_diskstats().unwrap_or_default();
        let fs_list    = filesystem::read_filesystems().unwrap_or_default();

        let devices: Vec<BlockDevice> = lsblk_devs.iter()
            .filter(|lb| !cfg.devices.exclude.iter().any(|pat| {
                if let Some(p) = pat.strip_suffix('*') { lb.name.starts_with(p) }
                else { pat == &lb.name }
            }))
            .filter(|lb| raw_stats.contains_key(&lb.name))
            .map(|lb| {
                let mut dev = BlockDevice::new(lb.name.clone());
                dev.model = lb.model.clone(); dev.serial = lb.serial.clone();
                dev.capacity_bytes = lb.size; dev.rotational = lb.rotational;
                dev.transport = lb.transport.clone(); dev.partitions = lb.partitions.clone();
                dev.infer_type();
                if smart_enabled { dev.smart = smart_collector::poll_device(&lb.name); }
                dev
            })
            .collect();

        let raids = collectors::mdraid::read_mdstat();
        let pools = collectors::zfs::read_zpools();
        let mut active = alerts::evaluate(&devices, &fs_list, &cfg.alerts);
        active.extend(alerts::evaluate_volumes(&raids, &pools));
        active.sort_by(|a, b| b.severity.cmp(&a.severity));

        let now  = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let bar  = "═".repeat(72);
        let secs_label = if interval_secs == 0 { "once".to_string() } else { format!("{}s", interval_secs) };
        println!("{}", bar);
        println!("  DTop  {}  (--watch {})", now, secs_label);
        println!("{}", bar);

        println!("\nDEVICES  ({} total)", devices.len());
        for dev in &devices {
            let temp    = dev.temperature().map(|t| format!("{}°C", t)).unwrap_or_else(|| "  —  ".to_string());
            let smart_s = dev.smart.as_ref()
                .map(|s| s.status.label().trim().to_string())
                .unwrap_or_else(|| "?".to_string());
            println!(
                "  {:<8}  {:<4}  R:{:>9}  W:{:>9}  util:{:>4.0}%  {:>5}  SMART:{:<5}  health:{}",
                dev.name, dev.dev_type.label().trim(),
                fmt_rate(dev.read_bytes_per_sec), fmt_rate(dev.write_bytes_per_sec),
                dev.io_util_pct, temp, smart_s, health_score(dev),
            );
        }

        println!("\nFILESYSTEMS  ({} total)", fs_list.len());
        for fs in &fs_list {
            let pct   = fs.use_pct();
            let alert = if pct >= 95.0 { " !!" } else if pct >= 85.0 { " !" } else { "" };
            let eta   = fs.days_until_full
                .map(|d| format!("  → full ~{:.0}d", d))
                .unwrap_or_default();
            println!(
                "  {:<20}  {:<6}  {:>8} / {:>8}  ({:>4.1}%){}{}",
                fs.mount, fs.fs_type,
                fmt_bytes(fs.used_bytes), fmt_bytes(fs.total_bytes),
                pct, alert, eta,
            );
        }

        if active.is_empty() {
            println!("\nALERTS  — none");
        } else {
            println!("\nALERTS  ({} active)", active.len());
            for a in &active {
                println!("  [{}] {}{}", a.severity.label(), a.prefix(), a.message);
            }
        }

        if let Some(psi) = collectors::pressure::read_pressure() {
            println!(
                "\nIO PRESSURE  some:{:.1}%  full:{:.1}%  (10s avg)",
                psi.io.some.avg10, psi.io.full.avg10
            );
        }

        println!();
        match tick {
            None    => break,
            Some(d) => std::thread::sleep(d),
        }
    }
    Ok(())
}

fn run_csv(smart_enabled: bool) -> Result<()> {
    use collectors::smart as smart_collector;
    use models::device::BlockDevice;
    use util::human::fmt_bytes;
    use util::health_score::health_score;

    let cfg = config::Config::load();
    let lsblk_devs = collectors::lsblk::run_lsblk().unwrap_or_default();
    let raw_stats  = collectors::diskstats::read_diskstats().unwrap_or_default();

    let devices: Vec<BlockDevice> = lsblk_devs.iter()
        .filter(|lb| !cfg.devices.exclude.iter().any(|pat| {
            if let Some(p) = pat.strip_suffix('*') { lb.name.starts_with(p) }
            else { pat == &lb.name }
        }))
        .filter(|lb| raw_stats.contains_key(&lb.name))
        .map(|lb| {
            let mut dev = BlockDevice::new(lb.name.clone());
            dev.model = lb.model.clone(); dev.serial = lb.serial.clone();
            dev.capacity_bytes = lb.size; dev.rotational = lb.rotational;
            dev.transport = lb.transport.clone(); dev.partitions = lb.partitions.clone();
            dev.infer_type();
            if smart_enabled { dev.smart = smart_collector::poll_device(&lb.name); }
            dev
        })
        .collect();

    println!("name,model,serial,type,capacity_bytes,capacity_hr,rotational,\
              read_bps,write_bps,util_pct,temp_c,smart_status,health_score");
    for dev in &devices {
        let model      = dev.model.as_deref().unwrap_or("").replace(',', ";");
        let serial     = dev.serial.as_deref().unwrap_or("").replace(',', ";");
        let smart_s    = dev.smart.as_ref().map(|s| s.status.label().trim().to_string())
                             .unwrap_or_else(|| "UNKNOWN".to_string());
        let temp       = dev.temperature().map(|t| t.to_string()).unwrap_or_default();
        let cap_hr     = fmt_bytes(dev.capacity_bytes);
        println!(
            "{},{},{},{},{},{},{},{:.0},{:.0},{:.1},{},{},{}",
            dev.name, model, serial,
            dev.dev_type.label().trim(),
            dev.capacity_bytes, cap_hr,
            dev.rotational,
            dev.read_bytes_per_sec, dev.write_bytes_per_sec,
            dev.io_util_pct,
            temp, smart_s,
            health_score(&dev),
        );
    }
    Ok(())
}

fn run_diff(file_a: &str, file_b: &str) -> Result<()> {
    use serde_json::Value;
    use util::human::fmt_bytes;

    let json_a: Value = serde_json::from_str(&std::fs::read_to_string(file_a)?)?;
    let json_b: Value = serde_json::from_str(&std::fs::read_to_string(file_b)?)?;

    let ts_a = json_a["timestamp"].as_str().unwrap_or("?");
    let ts_b = json_b["timestamp"].as_str().unwrap_or("?");
    println!("Comparing snapshots:");
    println!("  A: {} ({})", file_a, ts_a);
    println!("  B: {} ({})", file_b, ts_b);

    let empty: Vec<Value> = vec![];
    let devs_a = json_a["devices"].as_array().unwrap_or(&empty);
    let devs_b = json_b["devices"].as_array().unwrap_or(&empty);

    println!("\nDEVICES");
    for dev_b in devs_b {
        let name  = dev_b["name"].as_str().unwrap_or("?");
        let model = dev_b["model"].as_str().unwrap_or("");

        let dev_a = devs_a.iter().find(|d| d["name"].as_str() == Some(name));
        if dev_a.is_none() {
            println!("  {:<10} {}  [NEW]", name, model);
            continue;
        }
        let dev_a = dev_a.unwrap();
        let sm_a  = &dev_a["smart"];
        let sm_b  = &dev_b["smart"];

        let mut changes: Vec<String> = Vec::new();

        // SMART status
        if let (Some(s_a), Some(s_b)) = (sm_a["status"].as_str(), sm_b["status"].as_str()) {
            if s_a != s_b {
                changes.push(format!("SMART status:  {} → {}", s_a, s_b));
            }
        }

        // Temperature
        if let (Some(t_a), Some(t_b)) = (sm_a["temperature"].as_i64(), sm_b["temperature"].as_i64()) {
            if t_a != t_b {
                changes.push(format!("Temperature:   {}°C → {}°C  ({:+})", t_a, t_b, t_b - t_a));
            }
        }

        // Power-on hours
        if let (Some(p_a), Some(p_b)) = (sm_a["power_on_hours"].as_u64(), sm_b["power_on_hours"].as_u64()) {
            if p_a != p_b {
                changes.push(format!("Power-on hrs:  {} → {}  ({:+}h)", p_a, p_b, p_b as i64 - p_a as i64));
            }
        }

        // SMART attributes (raw value deltas)
        if let (Some(attrs_a), Some(attrs_b)) = (sm_a["attributes"].as_array(), sm_b["attributes"].as_array()) {
            for attr_b in attrs_b {
                let id     = attr_b["id"].as_u64().unwrap_or(0);
                let aname  = attr_b["name"].as_str().unwrap_or("?");
                let raw_b  = attr_b["raw_value"].as_u64().unwrap_or(0);

                if let Some(attr_a) = attrs_a.iter().find(|a| a["id"].as_u64() == Some(id)) {
                    let raw_a = attr_a["raw_value"].as_u64().unwrap_or(0);
                    if raw_a != raw_b {
                        changes.push(format!(
                            "Attr {:>3} {:<30} raw {} → {}  ({:+})",
                            id, format!("({})", aname), raw_a, raw_b, raw_b as i64 - raw_a as i64
                        ));
                    }
                } else {
                    changes.push(format!("Attr {:>3} ({})  [new] raw={}", id, aname, raw_b));
                }
            }
        }

        // Capacity change
        if let (Some(cap_a), Some(cap_b)) = (dev_a["capacity"].as_u64(), dev_b["capacity"].as_u64()) {
            if cap_a != cap_b {
                changes.push(format!("Capacity:  {} → {}", fmt_bytes(cap_a), fmt_bytes(cap_b)));
            }
        }

        if changes.is_empty() {
            println!("  {:<10} {}  (no changes)", name, model);
        } else {
            println!("  {:<10} {}", name, model);
            for c in &changes {
                println!("    {}", c);
            }
        }
    }

    for dev_a in devs_a {
        let name = dev_a["name"].as_str().unwrap_or("?");
        if !devs_b.iter().any(|d| d["name"].as_str() == Some(name)) {
            println!("  {:<10}  [REMOVED]", name);
        }
    }

    let fs_a = json_a["filesystems"].as_array().unwrap_or(&empty);
    let fs_b = json_b["filesystems"].as_array().unwrap_or(&empty);

    println!("\nFILESYSTEMS");
    for fsb in fs_b {
        let mp    = fsb["mountpoint"].as_str().unwrap_or("?");
        let pct_b = fsb["use_pct"].as_f64().unwrap_or(0.0);
        if let Some(fsa) = fs_a.iter().find(|f| f["mountpoint"].as_str() == Some(mp)) {
            let pct_a = fsa["use_pct"].as_f64().unwrap_or(0.0);
            let delta = pct_b - pct_a;
            if delta.abs() >= 0.1 {
                println!("  {:<24}  {:.0}% → {:.0}%  ({:+.1}pp)", mp, pct_a, pct_b, delta);
            } else {
                println!("  {:<24}  {:.0}%  (no change)", mp, pct_b);
            }
        } else {
            println!("  {:<24}  {:.0}%  [NEW]", mp, pct_b);
        }
    }
    for fsa in fs_a {
        let mp = fsa["mountpoint"].as_str().unwrap_or("?");
        if !fs_b.iter().any(|f| f["mountpoint"].as_str() == Some(mp)) {
            println!("  {:<24}  [REMOVED]", mp);
        }
    }

    Ok(())
}

fn run_completions(shell: &str) -> Result<()> {
    use clap::CommandFactory;
    use clap_complete::{generate, shells::{Bash, Elvish, Fish, PowerShell, Zsh}};

    let mut cmd = Cli::command();
    let mut out = io::stdout();
    match shell {
        "bash"       => generate(Bash,       &mut cmd, "dtop", &mut out),
        "zsh"        => generate(Zsh,        &mut cmd, "dtop", &mut out),
        "fish"       => generate(Fish,       &mut cmd, "dtop", &mut out),
        "elvish"     => generate(Elvish,     &mut cmd, "dtop", &mut out),
        "powershell" => generate(PowerShell, &mut cmd, "dtop", &mut out),
        other => {
            eprintln!("Unknown shell '{}'. Valid: bash, zsh, fish, elvish, powershell", other);
            std::process::exit(1);
        }
    }
    Ok(())
}

fn run(initial_theme: ui::theme::ThemeVariant, interval_ms: u64, smart_enabled: bool) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let mut app = App::new(initial_theme, interval_ms, smart_enabled)?;
    app.run(&mut term)?;

    Ok(())
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}
