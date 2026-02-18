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
        return run_alerts(cli.last);
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

    let snapshot = json!({
        "dtop_version": "0.1",
        "timestamp":    chrono::Local::now().to_rfc3339(),
        "devices":      devices,
        "filesystems":  filesystems,
        "nfs_mounts":   nfs_out,
        "raid_arrays":  raids_out,
        "zfs_pools":    pools_out,
        "psi":          psi_out,
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

fn run_alerts(n: usize) -> Result<()> {
    use util::alert_log;
    use alerts::Severity;

    let entries = alert_log::load_recent(n);
    if entries.is_empty() {
        println!("No alerts in log.");
        return Ok(());
    }
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
