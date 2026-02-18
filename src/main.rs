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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.json {
        return run_json_snapshot();
    }
    if cli.report {
        return run_report();
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
    use collectors::{filesystem, lsblk, nfs, smart_cache};
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
        json!({
            "name":        dev.name,
            "model":       dev.model,
            "serial":      dev.serial,
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

    let snapshot = json!({
        "dtop_version": "0.1",
        "timestamp": chrono::Local::now().to_rfc3339(),
        "devices":     devices,
        "filesystems": filesystems,
        "nfs_mounts":  nfs_out,
    });

    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}

fn run_report() -> Result<()> {
    use util::report;
    let (devices, filesystems) = report::collect_snapshot();
    let alerts = alerts::evaluate(&devices, &filesystems, &config::Config::load().alerts.thresholds);
    print!("{}", report::generate(&devices, &filesystems, &alerts));
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
    println!("  cooldown_hours        = {}", cfg.alerts.cooldown_hours);
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

    let active_alerts = alerts::evaluate(&devices, &fs_list, &cfg.alerts.thresholds);
    let has_crit = active_alerts.iter().any(|a| a.severity == Severity::Critical);
    let has_warn = active_alerts.iter().any(|a| a.severity == Severity::Warning);

    if active_alerts.is_empty() {
        println!("OK — {} device(s), {} filesystem(s), no alerts", devices.len(), fs_list.len());
        std::process::exit(0);
    }

    // Print all active alerts to stdout
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

        let new_alerts = alerts::evaluate(&devices, &fs_list, &cfg.alerts.thresholds);
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
