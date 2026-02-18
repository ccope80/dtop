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

    /// Number of entries to show (used with --alerts and --dmesg)
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

    /// Output file path for --report-html / --report-md (default: auto-named in current dir)
    #[arg(long, value_name = "FILE")]
    output: Option<String>,

    /// Only show alerts newer than this duration (e.g. 24h, 7d, 30m) — used with --alerts
    #[arg(long, value_name = "DURATION")]
    since: Option<String>,

    /// Show top processes by disk I/O (2-second sample) and exit
    #[arg(long)]
    top_io: bool,

    /// Row/sample limit: processes for --top-io, iterations for --iostat (0 = loop, default 10)
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

    /// View or set I/O scheduler: --io-sched (all), DEVICE (one), DEVICE=SCHEDULER (set)
    #[arg(long, value_name = "DEVICE[=SCHEDULER]", num_args = 0..=1, default_missing_value = "ALL")]
    io_sched: Option<String>,

    /// List devices by temperature, hottest first (reads SMART cache; no polling)
    #[arg(long)]
    top_temp: bool,

    /// Spin down an HDD to standby mode via hdparm (requires root + hdparm)
    #[arg(long, value_name = "DEVICE")]
    spindown: Option<String>,

    /// Use deep sleep (hdparm -Y) instead of standby (hdparm -y) with --spindown
    #[arg(long)]
    sleep_mode: bool,

    /// Run fstrim on a specific MOUNTPOINT, or all eligible filesystems if omitted
    #[arg(long, value_name = "MOUNTPOINT", num_args = 0..=1, default_missing_value = "ALL")]
    trim: Option<String>,

    /// View or set HDD APM level: DEVICE (view) or DEVICE=LEVEL (set, 1-254 power-save, 255=off)
    #[arg(long, value_name = "DEVICE[=LEVEL]")]
    apm: Option<String>,

    /// Generate a Markdown health report and exit (--output sets destination file)
    #[arg(long)]
    report_md: bool,

    /// Run a sequential read benchmark on DEVICE and exit (mirrors TUI 'b' key)
    #[arg(long, value_name = "DEVICE")]
    bench: Option<String>,

    /// Size of the sequential read in MiB (default 256, used with --bench)
    #[arg(long, default_value_t = 256)]
    bench_size: usize,

    /// Show health score history for DEVICE and exit
    #[arg(long, value_name = "DEVICE")]
    health_history: Option<String>,

    /// Days of history to display (default 7, used with --health-history; 0 = all)
    #[arg(long, default_value_t = 7)]
    days: usize,

    /// Sample filesystem fill rates and print a fill-forecast table, then exit
    #[arg(long)]
    forecast: bool,

    /// Rolling per-device I/O stats at 1-second intervals; optional DEVICE filter
    #[arg(long, value_name = "DEVICE", num_args = 0..=1, default_missing_value = "ALL")]
    iostat: Option<String>,

    /// Device capacity inventory table (lsblk + SMART cache, no polling)
    #[arg(long)]
    capacity: bool,

    /// Look up a single SMART attribute by ID or name substring: DEVICE ATTR
    #[arg(long, num_args = 2, value_names = ["DEVICE", "ATTR"])]
    smart_attr: Option<Vec<String>>,

    /// Print low-level sysfs device parameters for DEVICE
    #[arg(long, value_name = "DEVICE")]
    disk_info: Option<String>,

    /// Query HDD power state via hdparm -C; omit DEVICE to check all HDDs
    #[arg(long, value_name = "DEVICE", num_args = 0..=1, default_missing_value = "ALL")]
    power_state: Option<String>,

    /// Show cumulative I/O totals since boot (bytes, ops, avg latency) per device
    #[arg(long, value_name = "DEVICE", num_args = 0..=1, default_missing_value = "ALL")]
    cumulative_io: Option<String>,

    /// Show processes with open files on DEVICE or MOUNTPOINT (wraps lsof)
    #[arg(long, value_name = "DEVICE|MOUNT")]
    lsof: Option<String>,

    /// Print block device UUIDs, labels, and filesystem types (wraps blkid)
    #[arg(long)]
    blkid: bool,

    /// Print active mount table with key options (rw/ro, discard, errors, etc.)
    #[arg(long)]
    mount: bool,

    /// Show kernel storage messages from dmesg -T; optional DEVICE filter
    #[arg(long, value_name = "DEVICE", num_args = 0..=1, default_missing_value = "ALL")]
    dmesg: Option<String>,

    /// Read-verify DEVICE for I/O errors (dd conv=noerror,sync iflag=direct)
    #[arg(long, value_name = "DEVICE")]
    verify: Option<String>,

    /// Size in MiB to read for --verify (default 256)
    #[arg(long, default_value_t = 256)]
    size: usize,

    /// Show partition table for DEVICE augmented with UUID, FS type, and mount
    #[arg(long, value_name = "DEVICE")]
    partition_table: Option<String>,

    /// Display the SMART ATA/NVMe error log for DEVICE via smartctl
    #[arg(long, value_name = "DEVICE")]
    smart_errors: Option<String>,

    /// Show top directories by disk usage under PATH (default: current directory)
    #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    du: Option<String>,

    /// View or set the filesystem label for DEVICE; omit LABEL to print current
    #[arg(long, value_name = "DEVICE[=LABEL]")]
    label: Option<String>,

    /// Print current temperature for all devices from SMART cache (no polling)
    #[arg(long)]
    disk_temps: bool,

    /// Print model, serial, and firmware for all devices (or one DEVICE) from SMART cache
    #[arg(long, value_name = "DEVICE", num_args = 0..=1, default_missing_value = "ALL")]
    disk_model: Option<String>,

    /// Grow the filesystem on DEVICE to fill its partition (resize2fs/xfs_growfs/btrfs resize)
    #[arg(long, value_name = "DEVICE")]
    growfs: Option<String>,

    /// Start or check scrub status on DEVICE (btrfs/zfs/md-raid). Omit to check all.
    #[arg(long, value_name = "DEVICE", num_args = 0..=1, default_missing_value = "ALL")]
    scrub: Option<String>,

    /// Print redundancy status: which devices are in RAID/ZFS, which are bare
    #[arg(long)]
    redundancy: bool,

    /// Show TRIM/discard support and status for all SSDs and NVMe devices
    #[arg(long)]
    trim_report: bool,

    /// Print I/O pressure stall info (PSI) and per-device I/O wait stats
    #[arg(long)]
    io_pressure: bool,
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
    if let Some(arg) = &cli.io_sched {
        let target = if arg == "ALL" { None } else { Some(arg.as_str()) };
        return run_io_sched(target);
    }
    if cli.top_temp {
        return run_top_temp();
    }
    if let Some(dev) = &cli.spindown {
        return run_spindown(dev, cli.sleep_mode);
    }
    if let Some(mp_or_all) = &cli.trim {
        let mp = if mp_or_all == "ALL" { None } else { Some(mp_or_all.as_str()) };
        return run_trim(mp);
    }
    if let Some(arg) = &cli.apm {
        return run_apm(arg);
    }
    if cli.report_md {
        return run_report_md(cli.output.as_deref());
    }
    if let Some(dev) = &cli.bench {
        return run_cli_bench(dev, cli.bench_size);
    }
    if let Some(dev) = &cli.health_history {
        return run_health_history(dev, cli.days);
    }
    if cli.forecast {
        return run_forecast();
    }
    if let Some(dev_or_all) = &cli.iostat {
        let dev = if dev_or_all == "ALL" { None } else { Some(dev_or_all.as_str()) };
        return run_iostat(dev, cli.count);
    }
    if cli.capacity {
        return run_capacity();
    }
    if let Some(parts) = &cli.smart_attr {
        return run_smart_attr(&parts[0], &parts[1]);
    }
    if let Some(dev) = &cli.disk_info {
        return run_disk_info(dev);
    }
    if let Some(dev_or_all) = &cli.power_state {
        let dev = if dev_or_all == "ALL" { None } else { Some(dev_or_all.as_str()) };
        return run_power_state(dev);
    }
    if let Some(dev_or_all) = &cli.cumulative_io {
        let dev = if dev_or_all == "ALL" { None } else { Some(dev_or_all.as_str()) };
        return run_cumulative_io(dev);
    }
    if let Some(target) = &cli.lsof {
        return run_lsof(target);
    }
    if cli.blkid {
        return run_blkid();
    }
    if cli.mount {
        return run_mount();
    }
    if let Some(dev_or_all) = &cli.dmesg {
        let dev = if dev_or_all == "ALL" { None } else { Some(dev_or_all.as_str()) };
        return run_dmesg(dev, cli.last);
    }
    if let Some(dev) = &cli.verify {
        return run_verify(dev, cli.size);
    }
    if let Some(dev) = &cli.partition_table {
        return run_partition_table(dev);
    }
    if let Some(dev) = &cli.smart_errors {
        return run_smart_errors(dev);
    }
    if let Some(path) = &cli.du {
        return run_du(path);
    }
    if let Some(arg) = &cli.label {
        return run_label(arg);
    }
    if cli.disk_temps {
        return run_disk_temps();
    }
    if let Some(dev_or_all) = &cli.disk_model {
        let dev = if dev_or_all == "ALL" { None } else { Some(dev_or_all.as_str()) };
        return run_disk_model(dev);
    }
    if let Some(dev) = &cli.growfs {
        return run_growfs(dev);
    }
    if let Some(dev_or_all) = &cli.scrub {
        let dev = if dev_or_all == "ALL" { None } else { Some(dev_or_all.as_str()) };
        return run_scrub(dev);
    }
    if cli.redundancy {
        return run_redundancy();
    }
    if cli.trim_report {
        return run_trim_report();
    }
    if cli.io_pressure {
        return run_io_pressure();
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

fn run_trim(mountpoint: Option<&str>) -> Result<()> {
    let (args, desc): (Vec<&str>, String) = match mountpoint {
        None     => (vec!["-v", "-a"], "all eligible filesystems".to_string()),
        Some(mp) => (vec!["-v", mp],   format!("'{}'", mp)),
    };

    println!("Running fstrim on {}…", desc);

    let out = std::process::Command::new("fstrim")
        .args(&args)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("fstrim not found. It ships with util-linux (usually pre-installed).")
            } else {
                anyhow::anyhow!("Failed to run fstrim: {}", e)
            }
        })?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    if out.status.success() {
        if stdout.trim().is_empty() {
            println!("✓ TRIM completed.");
        } else {
            println!("{}", stdout.trim());
        }
    } else {
        if !stderr.trim().is_empty() { eprintln!("{}", stderr.trim()); }
        if !stdout.trim().is_empty() { eprintln!("{}", stdout.trim()); }
        if stderr.contains("Permission denied") || stdout.contains("Permission denied") {
            eprintln!("Hint: run as root to perform TRIM.");
        }
        std::process::exit(1);
    }
    Ok(())
}

fn apm_level_desc(level: u8) -> &'static str {
    match level {
        0        => "reserved",
        1..=127  => "aggressive power saving — spindown enabled",
        128      => "balanced — minimum power, no spindown",
        129..=253 => "performance — spindown disabled",
        254      => "maximum performance",
        255      => "APM feature disabled (always-on)",
    }
}

fn run_apm(arg: &str) -> Result<()> {
    if let Some((dev, level_str)) = arg.split_once('=') {
        let dev   = dev.trim_start_matches("/dev/");
        let level: u8 = level_str.trim().parse().map_err(|_| {
            anyhow::anyhow!(
                "Invalid APM level '{}'. Use 1-254 (power-save/spindown) or 255 (disable APM).",
                level_str
            )
        })?;

        let out = std::process::Command::new("hdparm")
            .args(["-B", &level.to_string(), &format!("/dev/{}", dev)])
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow::anyhow!("hdparm not found. Install: apt install hdparm")
                } else {
                    anyhow::anyhow!("Failed to run hdparm: {}", e)
                }
            })?;

        let stdout = String::from_utf8_lossy(&out.stdout);
        if out.status.success() {
            println!("✓ /dev/{} APM → {}  ({})", dev, level, apm_level_desc(level));
        } else {
            eprintln!("✗ hdparm failed (exit {}):\n{}", out.status, stdout.trim());
            if stdout.contains("Permission denied") || stdout.contains("HDIO") {
                eprintln!("  Hint: run as root to change APM settings.");
            }
            std::process::exit(1);
        }
    } else {
        let dev = arg.trim_start_matches("/dev/");
        let out = std::process::Command::new("hdparm")
            .args(["-B", &format!("/dev/{}", dev)])
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow::anyhow!("hdparm not found. Install: apt install hdparm")
                } else {
                    anyhow::anyhow!("Failed to run hdparm: {}", e)
                }
            })?;

        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.trim().is_empty() {
            println!("/dev/{}: No APM information (device may not support APM, or run as root).", dev);
        } else {
            println!("{}", stdout.trim());
        }
    }
    Ok(())
}

fn run_report_md(output: Option<&str>) -> Result<()> {
    use util::report;

    let cfg = config::Config::load();
    let (devices, filesystems) = report::collect_snapshot();
    let raids = collectors::mdraid::read_mdstat();
    let pools = collectors::zfs::read_zpools();
    let mut all_alerts = alerts::evaluate(&devices, &filesystems, &cfg.alerts);
    all_alerts.extend(alerts::evaluate_volumes(&raids, &pools));
    all_alerts.sort_by(|a, b| b.severity.cmp(&a.severity));
    let md = report::generate_markdown(&devices, &filesystems, &all_alerts, &raids, &pools);

    match output {
        Some(path) => {
            std::fs::write(path, &md)?;
            println!("Report written to: {}", path);
        }
        None => {
            let ts   = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let path = format!("dtop-report-{}.md", ts);
            std::fs::write(&path, &md)?;
            println!("Report written to: {}", path);
        }
    }
    Ok(())
}

fn read_scheduler(dev: &str) -> Option<(String, Vec<String>)> {
    let path = format!("/sys/block/{}/queue/scheduler", dev);
    let content = std::fs::read_to_string(path).ok()?;
    let mut active: Option<String> = None;
    let mut available: Vec<String> = Vec::new();
    for word in content.split_whitespace() {
        if word.starts_with('[') && word.ends_with(']') {
            let s = word.trim_matches(|c: char| c == '[' || c == ']').to_string();
            active = Some(s.clone());
            available.push(s);
        } else {
            available.push(word.to_string());
        }
    }
    active.map(|a| (a, available))
}

fn run_io_sched(arg: Option<&str>) -> Result<()> {
    // Enumerate real block devices from /sys/block (skip loop, optical, ram)
    let skip_prefixes = ["loop", "sr", "fd", "ram", "zram"];
    let mut all_devs: Vec<String> = std::fs::read_dir("/sys/block")?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if skip_prefixes.iter().any(|p| name.starts_with(p)) { return None; }
            let sched = format!("/sys/block/{}/queue/scheduler", name);
            if std::path::Path::new(&sched).exists() { Some(name) } else { None }
        })
        .collect();
    all_devs.sort();

    match arg {
        None => {
            // List all devices
            if all_devs.is_empty() {
                println!("No block devices with I/O scheduler control found.");
                return Ok(());
            }
            println!("{:<12}  {:<18}  {}", "Device", "Active", "Available");
            println!("{}", "─".repeat(62));
            for dev in &all_devs {
                if let Some((active, available)) = read_scheduler(dev) {
                    let others: Vec<&str> = available.iter()
                        .filter(|s| s.as_str() != active.as_str())
                        .map(String::as_str)
                        .collect();
                    let avail_str = if others.is_empty() {
                        "(only option)".to_string()
                    } else {
                        others.join("  ")
                    };
                    println!("{:<12}  {:<18}  {}", dev, active, avail_str);
                }
            }
            println!("\nSet with: dtop --io-sched DEVICE=SCHEDULER");
        }
        Some(a) if a.contains('=') => {
            // Set scheduler: DEVICE=SCHEDULER
            let (dev, sched) = a.split_once('=').unwrap();
            let dev = dev.trim_start_matches("/dev/");
            let sched = sched.trim();
            if sched.is_empty() {
                eprintln!("Scheduler name cannot be empty. Use DEVICE=SCHEDULER.");
                std::process::exit(1);
            }
            let sched_path = format!("/sys/block/{}/queue/scheduler", dev);
            if !std::path::Path::new(&sched_path).exists() {
                eprintln!("Device '{}' not found or has no scheduler control.", dev);
                std::process::exit(1);
            }
            match std::fs::write(&sched_path, sched) {
                Ok(_) => {
                    // Re-read to confirm
                    let confirmed = read_scheduler(dev)
                        .map(|(a, _)| a)
                        .unwrap_or_else(|| sched.to_string());
                    println!("✓ /dev/{} I/O scheduler → {}", dev, confirmed);
                }
                Err(e) => {
                    eprintln!("✗ Failed to set scheduler: {}", e);
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        eprintln!("  Hint: run as root to change I/O schedulers.");
                    }
                    std::process::exit(1);
                }
            }
        }
        Some(dev) => {
            // Show one device
            let dev = dev.trim_start_matches("/dev/");
            let sched_path = format!("/sys/block/{}/queue/scheduler", dev);
            if !std::path::Path::new(&sched_path).exists() {
                eprintln!("Device '{}' not found or has no scheduler control.", dev);
                std::process::exit(1);
            }
            if let Some((active, available)) = read_scheduler(dev) {
                println!("/dev/{}", dev);
                println!("  Active scheduler  : {}", active);
                println!("  Available         : {}", available.join("  "));
                println!();
                println!("  To change: dtop --io-sched {}=SCHEDULER", dev);
            }
        }
    }
    Ok(())
}

fn run_top_temp() -> Result<()> {
    use collectors::{lsblk, smart_cache};

    let cache = smart_cache::load();
    let devs  = lsblk::run_lsblk().unwrap_or_default();

    // Pair each device with its cached temperature
    let mut rows: Vec<(String, i32, &'static str, String)> = Vec::new();
    for dev in &devs {
        if let Some(smart) = cache.get(&dev.name) {
            if let Some(temp) = smart.temperature {
                let dtype = if dev.transport.as_deref().unwrap_or("").contains("nvme") {
                    "NVMe"
                } else if !dev.rotational {
                    "SSD"
                } else {
                    "HDD"
                };
                let model = dev.model.clone().unwrap_or_else(|| "?".to_string());
                rows.push((dev.name.clone(), temp, dtype, model));
            }
        }
    }

    if rows.is_empty() {
        println!("No temperature data in SMART cache.");
        println!("Run dtop (TUI) or dtop --daemon first to populate the cache,");
        println!("or use dtop --device-report DEVICE for an on-demand reading.");
        return Ok(());
    }

    rows.sort_by(|a, b| b.1.cmp(&a.1));  // hottest first
    let max_temp = rows[0].1.max(80);     // scale bar to at least 80°C

    println!("TEMPERATURE  ({} devices with SMART data)", rows.len());
    println!("{:<10}  {:>5}  {:<5}  {:<26}  {}",
        "Device", "Temp", "Type", "Model", "");
    println!("{}", "─".repeat(72));

    for (name, temp, dtype, model) in &rows {
        let is_hdd = *dtype == "HDD";
        let warn_t = if is_hdd { 50 } else { 55 };
        let crit_t = if is_hdd { 60 } else { 70 };
        let flag   = if *temp >= crit_t { " !!CRIT" } else if *temp >= warn_t { " !WARN" } else { "" };

        let bar_filled = ((*temp as f64 / max_temp as f64) * 20.0) as usize;
        let bar = format!("{}{}", "█".repeat(bar_filled), "░".repeat(20usize.saturating_sub(bar_filled)));

        let model_short: String = model.chars().take(26).collect();
        println!("{:<10}  {:>3}°C  {:<5}  {:<26}  {}{}",
            name, temp, dtype, model_short, bar, flag);
    }
    Ok(())
}

fn run_spindown(device: &str, sleep: bool) -> Result<()> {
    let name     = device.trim_start_matches("/dev/");
    let dev_path = format!("/dev/{}", name);
    let flag     = if sleep { "-Y" } else { "-y" };
    let mode     = if sleep { "sleep (deep)" } else { "standby" };

    println!("Setting /dev/{} to {} mode…", name, mode);
    if sleep {
        println!("  Warning: deep sleep requires a power cycle to spin the drive back up.");
    }

    let out = std::process::Command::new("hdparm")
        .args([flag, &dev_path])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "hdparm not found.\nInstall with:  apt install hdparm  (Debian/Ubuntu)\n\
                     or:            yum install hdparm  (RHEL/CentOS)"
                )
            } else {
                anyhow::anyhow!("Failed to run hdparm: {}", e)
            }
        })?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    if out.status.success() {
        println!("✓ /dev/{} is now in {} mode.", name, mode);
        if !stdout.trim().is_empty() { println!("{}", stdout.trim()); }
    } else {
        eprintln!("✗ hdparm exited {}:", out.status);
        if !stdout.trim().is_empty() { eprintln!("{}", stdout.trim()); }
        if !stderr.trim().is_empty() { eprintln!("{}", stderr.trim()); }
        if stderr.contains("Permission denied") || stdout.contains("HDIO_DRIVE_CMD") {
            eprintln!("  Hint: run as root to control drive power state.");
        }
        std::process::exit(1);
    }
    Ok(())
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

// ── --bench ──────────────────────────────────────────────────────────────────

fn run_cli_bench(device: &str, size_mib: usize) -> Result<()> {
    let dev_path = if device.starts_with("/dev/") {
        device.to_string()
    } else {
        format!("/dev/{}", device)
    };
    let display = device.trim_start_matches("/dev/");

    println!("Running {} MiB sequential read benchmark on {}…", size_mib, dev_path);
    let out = std::process::Command::new("dd")
        .args([
            format!("if={}", dev_path).as_str(),
            "of=/dev/null",
            "bs=1M",
            &format!("count={}", size_mib),
            "iflag=direct",
        ])
        .output()
        .map_err(|e| anyhow::anyhow!("dd failed: {}", e))?;

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let rate = bench_parse_dd_rate(&stderr).or_else(|| bench_parse_dd_rate(&stdout));

    match rate {
        Some(mbps) => println!("{}: {:.1} MB/s  ({} MiB sequential read, O_DIRECT)", display, mbps, size_mib),
        None => {
            eprintln!("Could not parse dd output:\n{}", stderr.trim());
            std::process::exit(1);
        }
    }
    Ok(())
}

fn bench_parse_dd_rate(s: &str) -> Option<f64> {
    // e.g. "268435456 bytes (268 MB, 256 MiB) copied, 1.23 s, 218 MB/s"
    let last = s.lines().last()?;
    let parts: Vec<&str> = last.split_whitespace().collect();
    for i in 1..parts.len() {
        let unit = parts[i];
        if unit.eq_ignore_ascii_case("MB/s") || unit.eq_ignore_ascii_case("MiB/s") {
            return parts[i - 1].parse::<f64>().ok();
        }
        if unit.eq_ignore_ascii_case("GB/s") || unit.eq_ignore_ascii_case("GiB/s") {
            return parts[i - 1].parse::<f64>().ok().map(|v| v * 1024.0);
        }
        if unit.eq_ignore_ascii_case("kB/s") || unit.eq_ignore_ascii_case("KiB/s") {
            return parts[i - 1].parse::<f64>().ok().map(|v| v / 1024.0);
        }
    }
    None
}

// ── --health-history ──────────────────────────────────────────────────────────

fn run_health_history(device: &str, days: usize) -> Result<()> {
    use util::health_history;

    let mut all = health_history::load();
    let name = device.trim_start_matches("/dev/");
    let scores = all.remove(name).unwrap_or_default();

    if scores.is_empty() {
        println!("No health history for '{}'. Run dtop or dtop --daemon to collect data.", name);
        return Ok(());
    }

    // Each entry ≈ one 5-min SMART poll interval → 288 entries/day
    let entries_per_day = 288usize;
    let max_entries = if days == 0 { scores.len() } else { (days * entries_per_day).max(1) };
    let start = scores.len().saturating_sub(max_entries);
    let recent = &scores[start..];

    let hours = recent.len() as f64 * 5.0 / 60.0;
    let min  = recent.iter().copied().min().unwrap_or(0);
    let max  = recent.iter().copied().max().unwrap_or(0);
    let avg  = recent.iter().copied().map(|v| v as f64).sum::<f64>() / recent.len() as f64;
    let cur  = *recent.last().unwrap_or(&0);

    println!("Health history — {}  ({} entries, ~{:.1}h)", name, recent.len(), hours);
    println!("  Current: {:>3}   Min: {:>3}   Max: {:>3}   Avg: {:.1}", cur, min, max, avg);
    println!("  Trend (oldest → newest):");
    println!("    {}", health_sparkline(recent, 64));
    println!("    ▁=low  ▄=50  █=100");

    // If small enough, also show a per-entry bar table
    if recent.len() <= 24 {
        println!();
        println!("  {:>4}  Score  Bar", "#");
        for (i, &s) in recent.iter().enumerate() {
            let bar_len = (s as usize * 30 / 100).min(30);
            let bar: String = "█".repeat(bar_len);
            println!("  {:>4}    {:>3}  {}", i + 1, s, bar);
        }
    }
    Ok(())
}

fn health_sparkline(scores: &[u8], width: usize) -> String {
    const BLOCKS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if scores.is_empty() { return String::new(); }

    if scores.len() <= width {
        scores.iter().map(|&s| BLOCKS[(s as usize * 8 / 100).min(8)]).collect()
    } else {
        (0..width).map(|i| {
            let lo = i * scores.len() / width;
            let hi = ((i + 1) * scores.len() / width).min(scores.len());
            let avg = scores[lo..hi].iter().map(|&v| v as f64).sum::<f64>() / (hi - lo) as f64;
            BLOCKS[(avg as usize * 8 / 100).min(8)]
        }).collect()
    }
}

// ── --forecast ────────────────────────────────────────────────────────────────

fn run_forecast() -> Result<()> {
    use collectors::filesystem;
    use util::human::fmt_bytes;

    print!("Sampling fill rates (2 s)…");
    use std::io::Write;
    std::io::stdout().flush()?;

    let snap1 = filesystem::read_filesystems()?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    let snap2 = filesystem::read_filesystems()?;
    let elapsed = 2.0f64;

    // Clear the sampling line
    println!("\r{:30}", "");

    println!("{:<28}  {:>8}  {:>8}  {:>6}  {:>10}  Est.Full",
        "Mount", "Size", "Avail", "Use%", "Fill Rate");
    println!("{}", "─".repeat(80));

    for fs2 in &snap2 {
        let fill_bps = snap1.iter()
            .find(|f| f.mount == fs2.mount)
            .map(|f| (fs2.used_bytes as f64 - f.used_bytes as f64) / elapsed);

        let rate_str = match fill_bps {
            None => "stable".to_string(),
            Some(f) if f.abs() < 512.0 => "stable".to_string(),
            Some(f) if f > 0.0 => format!("+{}/s", fmt_bytes(f as u64)),
            Some(f) => format!("-{}/s", fmt_bytes((-f) as u64)),
        };

        let eta_str = match fill_bps {
            Some(f) if f > 512.0 => {
                let days = fs2.avail_bytes as f64 / f / 86400.0;
                if days < 1.0      { format!("{:.0}h",  days * 24.0) }
                else if days < 30.0 { format!("{:.0}d",  days) }
                else if days < 365.0 { format!("{:.0}w", days / 7.0) }
                else               { format!("{:.1}y",  days / 365.0) }
            }
            _ => "—".to_string(),
        };

        println!("{:<28}  {:>8}  {:>8}  {:>5.1}%  {:>10}  {}",
            fs2.mount,
            fmt_bytes(fs2.total_bytes),
            fmt_bytes(fs2.avail_bytes),
            fs2.use_pct(),
            rate_str,
            eta_str,
        );
    }
    Ok(())
}

// ── --iostat ──────────────────────────────────────────────────────────────────

fn run_iostat(device: Option<&str>, count: usize) -> Result<()> {
    use collectors::diskstats;
    use util::human::fmt_bytes;

    let loop_forever = count == 0;
    let dev_filter = device.map(|d| d.trim_start_matches("/dev/").to_string());

    println!("{:<10}  {:>9}  {:>9}  {:>7}  {:>7}  {:>6}  {:>9}  {:>9}",
        "Device", "Read/s", "Write/s", "rIOPS", "wIOPS", "Util%", "rLat(ms)", "wLat(ms)");
    println!("{}", "─".repeat(80));

    let mut prev = diskstats::read_diskstats()?;
    let mut t0 = std::time::Instant::now();
    let mut iteration = 0usize;

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let curr = diskstats::read_diskstats()?;
        let elapsed = t0.elapsed().as_secs_f64();
        t0 = std::time::Instant::now();

        let ts = chrono::Local::now().format("%H:%M:%S");
        println!("── {} ─────────────────────────────────────────────────────────────────", ts);

        let mut dev_names: Vec<String> = curr.keys().cloned().collect();
        dev_names.sort();

        for dev in &dev_names {
            if let Some(ref f) = dev_filter {
                if dev != f { continue; }
            }
            if let (Some(p), Some(c)) = (prev.get(dev), curr.get(dev)) {
                let io = diskstats::compute_io(p, c, elapsed, 0);
                println!("{:<10}  {:>9}  {:>9}  {:>7.0}  {:>7.0}  {:>5.1}%  {:>9.2}  {:>9.2}",
                    dev,
                    fmt_bytes(io.read_bytes_per_sec as u64),
                    fmt_bytes(io.write_bytes_per_sec as u64),
                    io.read_iops,
                    io.write_iops,
                    io.io_util_pct,
                    io.avg_read_latency_ms,
                    io.avg_write_latency_ms,
                );
            }
        }

        prev = curr;
        iteration += 1;
        if !loop_forever && iteration >= count { break; }
    }
    Ok(())
}

// ── --capacity ────────────────────────────────────────────────────────────────

fn run_capacity() -> Result<()> {
    use collectors::{lsblk, smart_cache};
    use util::human::fmt_bytes;

    let disks  = lsblk::run_lsblk()?;
    let cache  = smart_cache::load();

    println!("{:<10}  {:>5}  {:>10}  {:<32}  {:>6}  {:>7}  {}",
        "Device", "Type", "Capacity", "Model", "POH", "SMART", "Serial");
    println!("{}", "─".repeat(88));

    let mut total: u64 = 0;

    for disk in &disks {
        let smart = cache.get(&disk.name);

        let dev_type = match disk.transport.as_deref() {
            Some("nvme") => "NVMe",
            _ if !disk.rotational => "SSD",
            _ => "HDD",
        };

        let model = disk.model.as_deref().unwrap_or("—");
        let model_trunc = if model.len() > 32 { &model[..32] } else { model };
        let serial = disk.serial.as_deref().unwrap_or("—");

        let poh_str = smart
            .and_then(|s| s.power_on_hours)
            .map(|h| format!("{}h", h))
            .unwrap_or_else(|| "—".to_string());

        let status_str = smart
            .map(|s| s.status.label().trim().to_string())
            .unwrap_or_else(|| "—".to_string());

        total += disk.size;

        println!("{:<10}  {:>5}  {:>10}  {:<32}  {:>6}  {:>7}  {}",
            disk.name, dev_type, fmt_bytes(disk.size),
            model_trunc, poh_str, status_str, serial,
        );
    }

    println!("{}", "─".repeat(88));
    println!("{:<10}  {:>5}  {:>10}  ({} device{})",
        "TOTAL", "", fmt_bytes(total), disks.len(), if disks.len() == 1 { "" } else { "s" });
    Ok(())
}

// ── --smart-attr ──────────────────────────────────────────────────────────────

fn run_smart_attr(device: &str, attr_query: &str) -> Result<()> {
    let dev_name = device.trim_start_matches("/dev/");
    let dev_path = format!("/dev/{}", dev_name);

    let out = std::process::Command::new("smartctl")
        .args(["--json=c", "-a", &dev_path])
        .output()
        .map_err(|e| anyhow::anyhow!("smartctl failed: {}", e))?;

    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| serde_json::json!({}));

    // ATA SMART attributes
    if let Some(attrs) = json["ata_smart_attributes"]["table"].as_array() {
        let query_id: Option<u64> = attr_query.parse().ok();
        let query_lc  = attr_query.to_lowercase();

        let matches: Vec<_> = attrs.iter().filter(|attr| {
            let id   = attr["id"].as_u64().unwrap_or(0);
            let name = attr["name"].as_str().unwrap_or("").to_lowercase();
            query_id.map_or(false, |q| q == id) || name.contains(&query_lc)
        }).collect();

        if matches.is_empty() {
            println!("No ATA SMART attribute matching '{}' on {}.", attr_query, dev_name);
            return Ok(());
        }

        println!("{:>3}  {:<36}  {:>5}  {:>5}  {:>5}  {:<16}  Prefail  Status",
            "ID", "Attribute", "Value", "Worst", "Thresh", "Raw");
        println!("{}", "─".repeat(90));

        for attr in matches {
            let id     = attr["id"].as_u64().unwrap_or(0);
            let name   = attr["name"].as_str().unwrap_or("?");
            let value  = attr["value"].as_u64().unwrap_or(0);
            let worst  = attr["worst"].as_u64().unwrap_or(0);
            let thresh = attr["thresh"].as_u64().unwrap_or(0);
            let raw_s  = attr["raw"]["string"].as_str().unwrap_or("?");
            let prefail = attr["flags"]["prefailure"].as_bool().unwrap_or(false);
            let failed  = attr["when_failed"].as_str().unwrap_or("-");

            let status = if failed != "-" && !failed.is_empty() { "FAILED" }
                else if prefail && thresh > 0 && value <= thresh + 10 { "AT RISK" }
                else { "OK" };

            println!("{:>3}  {:<36}  {:>5}  {:>5}  {:>5}  {:<16}  {:>7}  {}",
                id, name, value, worst, thresh, raw_s,
                if prefail { "yes" } else { "no" },
                status,
            );
        }
        return Ok(());
    }

    // NVMe — show health log fields
    if let Some(nvme) = json.get("nvme_smart_health_information_log") {
        let query_lc = attr_query.to_lowercase();
        println!("NVMe health log for {} (matching '{}'):", dev_name, attr_query);
        println!("{}", "─".repeat(60));
        let mut found = false;
        if let Some(obj) = nvme.as_object() {
            for (k, v) in obj {
                if k.to_lowercase().contains(&query_lc) {
                    println!("  {:<42}  {}", k, v);
                    found = true;
                }
            }
        }
        if !found {
            println!("  (no field matching '{}' — try 'temperature', 'spare', 'written')", attr_query);
        }
        return Ok(());
    }

    println!("No SMART attribute data found for {}.", dev_name);
    Ok(())
}

// ── --disk-info ───────────────────────────────────────────────────────────────

fn run_disk_info(device: &str) -> Result<()> {
    use util::human::fmt_bytes;

    let dev = device.trim_start_matches("/dev/");
    let base = format!("/sys/block/{}", dev);

    if !std::path::Path::new(&base).exists() {
        eprintln!("Device not found in sysfs: /sys/block/{}", dev);
        std::process::exit(1);
    }

    // Helpers to read a sysfs file, returning "—" on error
    let rd = |rel: &str| -> String {
        std::fs::read_to_string(format!("{}/{}", base, rel))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "—".to_string())
    };
    let q = |attr: &str| -> String {
        std::fs::read_to_string(format!("{}/queue/{}", base, attr))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "—".to_string())
    };

    // Capacity from sysfs size (unit = 512-byte sectors)
    let size_sectors: u64 = rd("size").parse().unwrap_or(0);
    let capacity_str = if size_sectors > 0 {
        format!("{} ({} sectors × 512 B)", fmt_bytes(size_sectors * 512), size_sectors)
    } else {
        "—".to_string()
    };

    // Discard / TRIM support
    let discard_max: u64 = q("discard_max_bytes").parse().unwrap_or(0);
    let discard_str = if discard_max > 0 {
        format!("yes  (max {})", fmt_bytes(discard_max))
    } else {
        "no".to_string()
    };

    // WBT latency target (0 = disabled)
    let wbt_raw = q("wbt_lat_usec");
    let wbt_str = match wbt_raw.parse::<u64>() {
        Ok(0) | Err(_) => "disabled".to_string(),
        Ok(us)         => format!("{} µs", us),
    };

    let rotational = q("rotational");
    let rot_str = if rotational == "1" { "yes (HDD)" } else { "no (SSD/NVMe)" };

    let removable = rd("removable");
    let rem_str = if removable == "1" { "yes" } else { "no" };

    println!("Device info — /dev/{}  (/sys/block/{})", dev, dev);
    println!("{}", "─".repeat(62));

    let row = |label: &str, value: &str| println!("  {:<32}  {}", label, value);

    row("Capacity",                  &capacity_str);
    row("Logical sector size",       &format!("{} B", q("logical_block_size")));
    row("Physical sector size",      &format!("{} B", q("physical_block_size")));
    row("HW sector size",            &format!("{} B", q("hw_sector_size")));
    row("Rotational",                rot_str);
    row("Removable",                 rem_str);
    println!();

    row("I/O scheduler",             &q("scheduler"));
    row("Queue depth (nr_requests)", &q("nr_requests"));
    row("Read-ahead",                &format!("{} KiB", q("read_ahead_kb")));
    row("Max request size",          &format!("{} KiB", q("max_sectors_kb")));
    row("Write cache",               &q("write_cache"));
    row("WBT latency target",        &wbt_str);
    println!();

    row("TRIM / discard support",    &discard_str);
    row("Zoned device",              &q("zoned"));
    row("DAX (direct access)",       &q("dax"));

    Ok(())
}

// ── --power-state ─────────────────────────────────────────────────────────────

fn run_power_state(device: Option<&str>) -> Result<()> {
    let skip = ["loop", "sr", "fd", "ram", "zram"];

    let devs: Vec<String> = if let Some(dev) = device {
        vec![dev.trim_start_matches("/dev/").to_string()]
    } else {
        // All rotational block devices
        let mut v: Vec<String> = std::fs::read_dir("/sys/block")
            .map_err(|e| anyhow::anyhow!("cannot read /sys/block: {}", e))?
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if skip.iter().any(|p| name.starts_with(p)) { return None; }
                let rotational = std::fs::read_to_string(
                    format!("/sys/block/{}/queue/rotational", name))
                    .map(|s| s.trim() == "1")
                    .unwrap_or(false);
                if rotational { Some(name) } else { None }
            })
            .collect();
        v.sort();
        v
    };

    if devs.is_empty() {
        println!("No HDD devices found. (SSDs and NVMe do not have a traditional power state.)");
        return Ok(());
    }

    println!("{:<12}  {}", "Device", "Power State");
    println!("{}", "─".repeat(36));

    for dev in &devs {
        let out = std::process::Command::new("hdparm")
            .args(["-C", &format!("/dev/{}", dev)])
            .output();

        let state = match out {
            Ok(o) => {
                let s = String::from_utf8_lossy(&o.stdout);
                s.lines()
                    .find(|l| l.contains("drive state is:"))
                    .and_then(|l| l.split(':').nth(1).map(|s| s.trim().to_string()))
                    .unwrap_or_else(|| "unknown".to_string())
            }
            Err(e) => format!("error: {}", e),
        };
        println!("{:<12}  {}", dev, state);
    }
    Ok(())
}

// ── --cumulative-io ───────────────────────────────────────────────────────────

fn run_cumulative_io(device: Option<&str>) -> Result<()> {
    use collectors::diskstats;
    use util::human::fmt_bytes;

    let raw = diskstats::read_diskstats()?;
    let dev_filter = device.map(|d| d.trim_start_matches("/dev/").to_string());

    let mut devs: Vec<(&String, &diskstats::RawDiskstat)> = raw.iter().collect();
    devs.sort_by(|a, b| a.0.cmp(b.0));

    println!("{:<10}  {:>13}  {:>13}  {:>10}  {:>10}  {:>8}  {:>9}  {:>9}",
        "Device", "Total Read", "Total Written", "Read Ops", "Write Ops",
        "In-Flight", "Avg rLat", "Avg wLat");
    println!("{}", "─".repeat(96));

    for (dev, stat) in &devs {
        if let Some(ref f) = dev_filter {
            if *dev != f { continue; }
        }

        let read_bytes  = stat.sectors_read    * 512;
        let write_bytes = stat.sectors_written * 512;
        let avg_r_ms = if stat.reads_completed  > 0 {
            stat.ms_reading as f64 / stat.reads_completed  as f64
        } else { 0.0 };
        let avg_w_ms = if stat.writes_completed > 0 {
            stat.ms_writing as f64 / stat.writes_completed as f64
        } else { 0.0 };

        println!("{:<10}  {:>13}  {:>13}  {:>10}  {:>10}  {:>8}  {:>8.2}ms  {:>8.2}ms",
            dev,
            fmt_bytes(read_bytes),
            fmt_bytes(write_bytes),
            stat.reads_completed,
            stat.writes_completed,
            stat.ios_in_progress,
            avg_r_ms,
            avg_w_ms,
        );
    }
    Ok(())
}

// ── --lsof ────────────────────────────────────────────────────────────────────

fn run_lsof(target: &str) -> Result<()> {
    // Determine whether target is a device node or a directory/mount
    let (flag, display) = if target.starts_with("/dev/") {
        // lsof <device>  — list processes with the device open
        (target.to_string(), format!("device {}", target))
    } else {
        // lsof +D <mountpoint>  — recursively find open files under path
        (target.to_string(), format!("mount {}", target))
    };

    let mut cmd = std::process::Command::new("lsof");
    if target.starts_with("/dev/") {
        cmd.arg(&flag);
    } else {
        cmd.args(["+D", &flag]);
    }

    let out = cmd.output()
        .map_err(|e| anyhow::anyhow!("lsof failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() || (lines.len() == 1 && lines[0].starts_with("COMMAND")) {
        println!("No processes with open files on {}.", display);
        return Ok(());
    }

    // Print header then deduplicated rows (lsof can repeat entries for mmap regions etc.)
    println!("Open files on {} ({} entries):", display, lines.len().saturating_sub(1));
    println!("{}", "─".repeat(90));

    // Track (pid, command, fd, name) to deduplicate mem-mapped duplicates
    let mut seen = std::collections::HashSet::new();

    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            // Reformatted header
            println!("{:<16}  {:>7}  {:<10}  {:<6}  {:<8}  {}",
                "Command", "PID", "User", "FD", "Type", "Name");
            println!("{}", "─".repeat(90));
            continue;
        }
        // split_whitespace gives clean tokens; NAME is everything from index 8 onward
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 9 { continue; }
        let (cmd, pid, user, fd, ftype) = (t[0], t[1], t[2], t[3], t[4]);
        // t[5]=DEVICE, t[6]=SIZE/OFF, t[7]=NODE, t[8..]=NAME (may contain spaces)
        let name = t[8..].join(" ");

        // Skip memory-mapped/deleted duplicates
        if ftype == "MEM" || ftype == "DEL" { continue; }

        let key = format!("{}-{}-{}-{}", pid, cmd, fd, name);
        if !seen.insert(key) { continue; }

        let cmd_short = if cmd.len() > 15 { &cmd[..15] } else { cmd };
        println!("{:<16}  {:>7}  {:<10}  {:<6}  {:<8}  {}",
            cmd_short, pid, user, fd, ftype, name);
    }
    Ok(())
}

// ── --blkid ───────────────────────────────────────────────────────────────────

fn run_blkid() -> Result<()> {
    let out = std::process::Command::new("blkid")
        .output()
        .map_err(|e| anyhow::anyhow!("blkid failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&out.stdout);

    // Parse "key=value" pairs; skip loop devices
    struct BlkEntry {
        device: String,
        label:  String,
        uuid:   String,
        fs_type: String,
        partuuid: String,
    }

    let mut entries: Vec<BlkEntry> = Vec::new();

    for line in stdout.lines() {
        let (dev, rest) = match line.split_once(':') {
            Some(p) => p,
            None => continue,
        };
        let dev = dev.trim();
        if dev.starts_with("/dev/loop") { continue; }

        let get = |key: &str| -> String {
            // Find KEY="value" in rest
            let needle = format!("{}=\"", key);
            rest.find(&needle)
                .and_then(|i| {
                    let after = &rest[i + needle.len()..];
                    after.find('"').map(|j| after[..j].to_string())
                })
                .unwrap_or_default()
        };

        entries.push(BlkEntry {
            device:   dev.to_string(),
            label:    get("LABEL"),
            uuid:     get("UUID"),
            fs_type:  get("TYPE"),
            partuuid: get("PARTUUID"),
        });
    }

    if entries.is_empty() {
        println!("No block devices found (run as root for complete output).");
        return Ok(());
    }

    // Sort: whole disks first, then partitions
    entries.sort_by(|a, b| a.device.cmp(&b.device));

    println!("{:<16}  {:<14}  {:<38}  {:<10}  {}",
        "Device", "Label", "UUID", "Type", "PARTUUID");
    println!("{}", "─".repeat(96));

    for e in &entries {
        let label    = if e.label.is_empty()    { "—".to_string() } else { e.label.clone() };
        let uuid     = if e.uuid.is_empty()     { "—".to_string() } else { e.uuid.clone() };
        let fs_type  = if e.fs_type.is_empty()  { "—".to_string() } else { e.fs_type.clone() };
        let partuuid = if e.partuuid.is_empty() { "—".to_string() } else { e.partuuid.clone() };
        println!("{:<16}  {:<14}  {:<38}  {:<10}  {}",
            e.device, label, uuid, fs_type, partuuid);
    }
    Ok(())
}

// ── --mount ───────────────────────────────────────────────────────────────────

fn run_mount() -> Result<()> {
    const SKIP_FS: &[&str] = &[
        "proc", "sysfs", "devpts", "tmpfs", "devtmpfs", "cgroup", "cgroup2",
        "pstore", "efivarfs", "securityfs", "debugfs", "tracefs", "bpf",
        "hugetlbfs", "mqueue", "fusectl", "configfs", "binfmt_misc",
        "overlay", "nsfs", "rpc_pipefs", "autofs", "squashfs",
    ];
    const SKIP_PREFIX: &[&str] = &[
        "/proc", "/sys", "/dev/pts", "/run/user", "/snap",
    ];

    // Important options to surface (in this priority order)
    let important_opts = |opts: &str| -> String {
        let mut keep: Vec<&str> = Vec::new();
        for opt in opts.split(',') {
            match opt {
                "rw" | "ro" | "noexec" | "nosuid" | "nodev"
                | "discard" | "relatime" | "noatime" | "strictatime"
                | "bind" | "rbind" => keep.push(opt),
                o if o.starts_with("errors=") => keep.push(o),
                o if o.starts_with("commit=") => keep.push(o),
                o if o.starts_with("mode=")   => keep.push(o),
                _ => {}
            }
        }
        keep.join(",")
    };

    let content = std::fs::read_to_string("/proc/mounts")?;
    let mut rows: Vec<(String, String, String, String, bool)> = Vec::new();

    for line in content.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 { continue; }
        let (dev, mount, fstype, opts) = (f[0], f[1], f[2], f[3]);

        if SKIP_FS.contains(&fstype) { continue; }
        if SKIP_PREFIX.iter().any(|p| mount.starts_with(p)) { continue; }
        if dev.starts_with("/dev/loop") { continue; }

        let ro = opts.split(',').any(|o| o == "ro");
        let key_opts = important_opts(opts);
        rows.push((dev.to_string(), mount.to_string(), fstype.to_string(), key_opts, ro));
    }

    rows.sort_by(|a, b| a.1.cmp(&b.1));

    println!("{:<22}  {:<24}  {:<8}  {}",
        "Device", "Mount", "FS", "Options");
    println!("{}", "─".repeat(80));

    for (dev, mount, fstype, opts, ro) in &rows {
        let dev_short = dev.trim_start_matches("/dev/");
        let ro_marker = if *ro { " [RO]" } else { "" };
        println!("{:<22}  {:<24}  {:<8}  {}{}",
            dev_short, mount, fstype, opts, ro_marker);
    }
    Ok(())
}

// ── --dmesg ───────────────────────────────────────────────────────────────────

fn run_dmesg(device: Option<&str>, last: usize) -> Result<()> {
    const STORAGE_PAT: &[&str] = &[
        "I/O error", "blk_update_request", "Buffer I/O",
        "ata", "scsi", "nvme", "virtio_scsi",
        " sd ", "sda", "sdb", "sdc", "sdd",
        "EXT4-fs", "XFS", "BTRFS", "jbd2", "ext4",
        "hard resetting link", "Exception Emask", "failed command",
        "reset failed", "medium error", "sense key", "disk error",
        "SCSI error", "Unrecovered read error",
    ];

    let out = std::process::Command::new("dmesg")
        .arg("-T")
        .output()
        .map_err(|e| anyhow::anyhow!("dmesg failed: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout);

    let dev_name = device.map(|d| d.trim_start_matches("/dev/"));
    let display  = dev_name
        .map(|d| format!("device '{}'", d))
        .unwrap_or_else(|| "all storage events".to_string());

    let matched: Vec<&str> = stdout.lines().filter(|line| {
        match dev_name {
            Some(dev) => line.contains(dev),
            None      => STORAGE_PAT.iter().any(|p| line.contains(p)),
        }
    }).collect();

    let total = matched.len();
    let skip  = total.saturating_sub(last);
    let shown = &matched[skip..];

    if shown.is_empty() {
        println!("No kernel messages found for {}.", display);
        return Ok(());
    }

    println!("dmesg — {}  (showing {} of {} matching lines)", display, shown.len(), total);
    println!("{}", "─".repeat(80));
    for line in shown {
        println!("{}", line);
    }
    Ok(())
}

// ── --verify ──────────────────────────────────────────────────────────────────

fn run_verify(device: &str, size_mib: usize) -> Result<()> {
    let dev_path = if device.starts_with("/dev/") {
        device.to_string()
    } else {
        format!("/dev/{}", device)
    };
    let block_count = size_mib * 2; // bs=512K → 2 blocks per MiB

    println!("Read-verify: {} MiB from {}  (O_DIRECT, conv=noerror,sync)", size_mib, dev_path);
    println!("Bad blocks will be reported below; replaced with zeros in output stream.");
    println!("Running…");

    let t0  = std::time::Instant::now();
    let out = std::process::Command::new("dd")
        .args([
            format!("if={}", dev_path).as_str(),
            "of=/dev/null",
            "bs=512K",
            &format!("count={}", block_count),
            "conv=noerror,sync",
            "iflag=direct",
        ])
        .output()
        .map_err(|e| anyhow::anyhow!("dd failed: {}", e))?;

    let elapsed = t0.elapsed().as_secs_f64();
    let stderr  = String::from_utf8_lossy(&out.stderr);
    let stdout  = String::from_utf8_lossy(&out.stdout);

    // Error lines: contain "error" but aren't the final summary
    let errors: Vec<&str> = stderr.lines().chain(stdout.lines())
        .filter(|l| l.to_lowercase().contains("error") && !l.contains("records"))
        .collect();

    // Summary line: "N bytes ... copied, N s, N MB/s"
    let summary = stderr.lines().chain(stdout.lines())
        .filter(|l| l.contains("bytes") && l.contains("copied"))
        .last()
        .unwrap_or("(no summary from dd)");

    println!();
    if errors.is_empty() {
        println!("Result:  No I/O errors detected ✓");
    } else {
        println!("Result:  I/O ERRORS DETECTED  ({} error line(s))", errors.len());
    }
    println!("Elapsed: {:.1}s", elapsed);
    println!("dd:      {}", summary);

    if !errors.is_empty() {
        println!();
        println!("Error details:");
        for e in &errors {
            println!("  {}", e);
        }
    }
    Ok(())
}

// ── --partition-table ─────────────────────────────────────────────────────────

fn extract_quoted(text: &str, key: &str) -> String {
    let needle = format!("{}=\"", key);
    text.find(&needle)
        .and_then(|i| {
            let s = &text[i + needle.len()..];
            s.find('"').map(|j| s[..j].to_string())
        })
        .unwrap_or_default()
}

fn run_partition_table(device: &str) -> Result<()> {
    let dev_path = if device.starts_with("/dev/") {
        device.to_string()
    } else {
        format!("/dev/{}", device)
    };

    // /proc/mounts: device → mountpoint
    let mounts: std::collections::HashMap<String, String> =
        std::fs::read_to_string("/proc/mounts")
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let mut f = l.split_whitespace();
                let dev = f.next()?.to_string();
                let mnt = f.next()?.to_string();
                Some((dev, mnt))
            })
            .collect();

    // blkid: device → (uuid, fstype)
    let blkid_raw = std::process::Command::new("blkid")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let blkid: std::collections::HashMap<String, (String, String)> = blkid_raw
        .lines()
        .filter_map(|line| {
            let (dev, rest) = line.split_once(':')?;
            let uuid   = extract_quoted(rest, "UUID");
            let fstype = extract_quoted(rest, "TYPE");
            Some((dev.trim().to_string(), (uuid, fstype)))
        })
        .collect();

    // fdisk -l for partition layout
    let fdisk_out = std::process::Command::new("fdisk")
        .args(["-l", &dev_path])
        .output()
        .map_err(|e| anyhow::anyhow!("fdisk failed: {}", e))?;
    let fdisk_str = String::from_utf8_lossy(&fdisk_out.stdout);

    let mut past_header = false;
    for line in fdisk_str.lines() {
        if line.starts_with("Device") {
            past_header = true;
            println!();
            println!("{:<16}  {:>6}  {:<8}  {:<36}  {:<14}  {}",
                "Partition", "Size", "FS", "UUID", "Type", "Mount");
            println!("{}", "─".repeat(96));
            continue;
        }
        if !past_header {
            if !line.trim().is_empty() { println!("{}", line); }
            continue;
        }
        if line.trim().is_empty() || line.starts_with("Partition table") { continue; }

        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() || !t[0].starts_with('/') { continue; }

        let part  = t[0];
        let size  = t.get(4).copied().unwrap_or("?");
        let ptype = if t.len() > 5 { t[5..].join(" ") } else { "?".to_string() };
        let ptype_short = if ptype.len() > 14 { format!("{}..", &ptype[..12]) } else { ptype };

        let (uuid, fstype) = blkid.get(part)
            .map(|(u, t)| (u.as_str(), t.as_str()))
            .unwrap_or(("—", "—"));
        let mount = mounts.get(part).map(|s| s.as_str()).unwrap_or("—");

        println!("{:<16}  {:>6}  {:<8}  {:<36}  {:<14}  {}", part, size, fstype, uuid, ptype_short, mount);
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

fn run_smart_errors(device: &str) -> Result<()> {
    use serde_json::Value;

    let name     = device.trim_start_matches("/dev/");
    let dev_path = format!("/dev/{}", name);

    println!("SMART Error Log — {}\n", dev_path);

    let out = std::process::Command::new("smartctl")
        .args(["--json=c", "-l", "error", &dev_path])
        .output()
        .map_err(|e| anyhow::anyhow!("smartctl failed: {}\nIs smartctl installed?", e))?;

    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);

    // ── ATA error log ──────────────────────────────────────────────────
    if let Some(log) = v.get("ata_smart_error_log") {
        let section = if log["extended"]["table"].is_array() { "extended" } else { "summary" };
        let total   = log[section]["count"].as_u64().unwrap_or(0);
        let table   = log[section]["table"].as_array();

        if total == 0 || table.map_or(true, |t| t.is_empty()) {
            println!("  No ATA errors logged — drive is healthy.");
            return Ok(());
        }

        println!("  ATA Error Count: {total}\n");
        println!("  {:>5}  {:>8}  {:>14}  {:<10}  Command", "Err#", "Hours", "LBA", "Error");
        println!("  {}  {}  {}  {}  {}", "─".repeat(5), "─".repeat(8), "─".repeat(14), "─".repeat(10), "─".repeat(30));

        for entry in table.into_iter().flatten() {
            let err_num = entry["error_number"].as_u64().unwrap_or(0);
            let hours   = entry["lifetime_hours"].as_u64().unwrap_or(0);
            let lba     = entry["lba"].as_u64();
            let err_str = entry["error_register"]["string"]
                .as_str()
                .unwrap_or("?");
            let cmd_str = entry["previous_commands"][0]["command_name"]
                .as_str()
                .unwrap_or("?");

            let lba_s = match lba {
                Some(l) => format!("0x{:012x}", l),
                None    => "—".to_string(),
            };
            println!("  {:>5}  {:>7}h  {:>14}  {:<10}  {}", err_num, hours, lba_s, err_str, cmd_str);
        }

        if let Some(t) = table {
            if (total as usize) > t.len() {
                println!("\n  (Device log holds only the most recent {} entries; {} total recorded.)", t.len(), total);
            }
        }
        return Ok(());
    }

    // ── NVMe — show health log error summary ───────────────────────────
    if let Some(h) = v.get("nvme_smart_health_information_log") {
        let err_entries = h["num_err_log_entries"].as_u64().unwrap_or(0);
        let media_errs  = h["media_errors"].as_u64().unwrap_or(0);
        let crit_warn   = h["critical_warning"].as_u64().unwrap_or(0);
        println!("  NVMe Error Summary:");
        println!("  Error log entries : {}", err_entries);
        println!("  Media/data errors : {}", media_errs);
        println!("  Critical warning  : 0x{:02x}", crit_warn);
        if err_entries == 0 && media_errs == 0 && crit_warn == 0 {
            println!("\n  No errors detected — drive is healthy.");
        } else {
            println!("\n  Use 'nvme error-log {}' for the full NVMe error log.", dev_path);
        }
        return Ok(());
    }

    // ── Fallback: raw smartctl text ───────────────────────────────────
    let raw = std::process::Command::new("smartctl")
        .args(["-l", "error", &dev_path])
        .output()?;
    let text = String::from_utf8_lossy(&raw.stdout);
    for line in text.lines().skip(4) {
        println!("  {}", line);
    }
    Ok(())
}

fn run_du(path: &str) -> Result<()> {
    let out = std::process::Command::new("du")
        .args(["-ahd1", "--", path])
        .output()
        .map_err(|e| anyhow::anyhow!("du failed: {}", e))?;

    if !out.status.success() && out.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("{}", stderr.trim());
    }

    struct DuEntry { bytes: u64, raw_size: String, path: String }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut entries: Vec<DuEntry> = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(2, '\t');
        let size_str = match parts.next() { Some(s) => s.trim(), None => continue };
        let path_str = match parts.next() { Some(p) => p.trim(), None => continue };
        let bytes = parse_du_size(size_str);
        entries.push(DuEntry { bytes, raw_size: size_str.to_string(), path: path_str.to_string() });
    }

    entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let entries: Vec<_> = entries.into_iter().take(20).collect();
    let max_bytes = entries.first().map_or(1, |e| e.bytes.max(1));

    println!("Disk Usage — {}\n", path);
    println!("{:>8}  {:<20}  Path", "Size", "Usage");
    println!("{}", "─".repeat(80));

    const BAR_W: usize = 20;
    for e in &entries {
        let filled = ((e.bytes as f64 / max_bytes as f64) * BAR_W as f64).round() as usize;
        let filled = filled.min(BAR_W);
        let bar    = format!("{}{}", "█".repeat(filled), "░".repeat(BAR_W - filled));
        let display_path = if e.path.len() > 50 {
            format!("…{}", &e.path[e.path.len().saturating_sub(49)..])
        } else {
            e.path.clone()
        };
        println!("{:>8}  {}  {}", e.raw_size, bar, display_path);
    }
    Ok(())
}

fn parse_du_size(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() { return 0; }
    // GNU du -h uses suffixes like "1.5G", "512M", "100K", or bare bytes
    let last = s.chars().last().unwrap_or('0');
    let num_part = &s[..s.len() - last.len_utf8()];
    let n: f64 = num_part.parse().unwrap_or(0.0);
    match last {
        'G' => (n * 1_073_741_824.0) as u64,
        'M' => (n * 1_048_576.0)     as u64,
        'K' => (n * 1_024.0)         as u64,
        'T' => (n * 1_099_511_627_776.0) as u64,
        _   => s.parse().unwrap_or(0),
    }
}

fn run_label(arg: &str) -> Result<()> {
    // Accept "DEV" (view) or "DEV=LABEL" (set)
    let (dev_raw, new_label) = if let Some((d, l)) = arg.split_once('=') {
        (d, Some(l))
    } else {
        (arg, None)
    };
    let name     = dev_raw.trim_start_matches("/dev/");
    let dev_path = format!("/dev/{}", name);

    // Detect filesystem type via blkid
    let blkid_out = std::process::Command::new("blkid")
        .args(["-o", "value", "-s", "TYPE", &dev_path])
        .output()
        .map_err(|e| anyhow::anyhow!("blkid failed: {}", e))?;
    let fstype = String::from_utf8_lossy(&blkid_out.stdout).trim().to_string();

    if let Some(label) = new_label {
        // Set label
        let result = match fstype.as_str() {
            "ext2" | "ext3" | "ext4" => std::process::Command::new("e2label")
                .args([&dev_path, label])
                .status(),
            "xfs" => std::process::Command::new("xfs_admin")
                .args(["-L", label, &dev_path])
                .status(),
            "btrfs" => std::process::Command::new("btrfs")
                .args(["filesystem", "label", &dev_path, label])
                .status(),
            "ntfs" => std::process::Command::new("ntfslabel")
                .args([&dev_path, label])
                .status(),
            "vfat" | "fat32" | "fat16" => std::process::Command::new("fatlabel")
                .args([&dev_path, label])
                .status(),
            _ => {
                anyhow::bail!("Unsupported filesystem type '{}' for label set (detected on {})", fstype, dev_path);
            }
        };
        match result {
            Ok(s) if s.success() => println!("Label set to '{}' on {} ({})", label, dev_path, fstype),
            Ok(s) => anyhow::bail!("Label command exited {}", s),
            Err(e) => anyhow::bail!("Failed to run label tool: {}", e),
        }
    } else {
        // View label
        let blkid_label = std::process::Command::new("blkid")
            .args(["-o", "value", "-s", "LABEL", &dev_path])
            .output()
            .map_err(|e| anyhow::anyhow!("blkid failed: {}", e))?;
        let label = String::from_utf8_lossy(&blkid_label.stdout).trim().to_string();
        println!("Device  : {}", dev_path);
        println!("FS type : {}", if fstype.is_empty() { "unknown" } else { &fstype });
        println!("Label   : {}", if label.is_empty() { "(none)" } else { &label });
    }
    Ok(())
}

fn run_disk_temps() -> Result<()> {
    use crate::collectors::smart_cache;

    let cache = smart_cache::load();
    if cache.is_empty() {
        println!("No SMART data cached yet — run dtop (TUI) first to populate the cache.");
        return Ok(());
    }

    // Collect devices with temperature data, sorted hottest first
    let mut rows: Vec<(String, i32)> = cache.iter()
        .filter_map(|(name, data)| data.temperature.map(|t| (name.clone(), t)))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));

    println!("{:<12} {:>7}  Gauge", "Device", "Temp");
    println!("{}", "─".repeat(40));

    const BAR_W: usize = 20;
    for (name, temp) in &rows {
        // Gauge: 20-85°C range maps to full bar
        let pct  = ((*temp as f64 - 20.0) / 65.0).clamp(0.0, 1.0);
        let fill = (pct * BAR_W as f64).round() as usize;
        let bar  = format!("{}{}", "█".repeat(fill.min(BAR_W)), "░".repeat(BAR_W - fill.min(BAR_W)));
        let indicator = if *temp >= 70 { "▲ HOT" } else if *temp >= 55 { "  warm" } else { "  ok" };
        println!("{:<12} {:>5}°C  {}  {}", name, temp, bar, indicator);
    }

    if rows.is_empty() {
        println!("  No temperature data in SMART cache.");
    }
    Ok(())
}

fn run_disk_model(device: Option<&str>) -> Result<()> {
    use crate::collectors::smart_cache;

    let cache = smart_cache::load();

    // Also gather sysfs model/vendor info for any device not in cache
    let mut sysfs_names: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/sys/block") {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("loop") || name.starts_with("ram") { continue; }
            sysfs_names.push(name);
        }
    }
    sysfs_names.sort();

    struct Row { name: String, model: String, serial: String, firmware: String, capacity: String }
    let mut rows: Vec<Row> = Vec::new();

    let names: Vec<String> = match device {
        Some(d) => vec![d.trim_start_matches("/dev/").to_string()],
        None    => {
            let mut all: std::collections::HashSet<String> = sysfs_names.iter().cloned().collect();
            for k in cache.keys() { all.insert(k.clone()); }
            let mut v: Vec<String> = all.into_iter().collect();
            v.sort();
            v
        }
    };

    for name in &names {
        let _smart = cache.get(name);

        // sysfs paths
        let sysfs_base = format!("/sys/block/{}", name);
        let read_sysfs = |sub: &str| -> String {
            std::fs::read_to_string(format!("{}/{}", sysfs_base, sub))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };

        // Try smartctl JSON for accurate model/serial/firmware
        let smartctl_out = std::process::Command::new("smartctl")
            .args(["--json=c", "-i", &format!("/dev/{}", name)])
            .output();

        let (model, serial, firmware, capacity) = if let Ok(out) = smartctl_out {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                let model    = v["model_name"].as_str().unwrap_or("").to_string();
                let serial   = v["serial_number"].as_str().unwrap_or("").to_string();
                let firmware = v["firmware_version"].as_str().unwrap_or("").to_string();
                let cap_bytes = v["user_capacity"]["bytes"].as_u64().unwrap_or(0);
                let cap = if cap_bytes > 0 {
                    crate::util::human::fmt_bytes(cap_bytes)
                } else {
                    String::new()
                };
                (model, serial, firmware, cap)
            } else {
                (read_sysfs("device/model"), String::new(), read_sysfs("device/rev"), String::new())
            }
        } else {
            (read_sysfs("device/model"), String::new(), read_sysfs("device/rev"), String::new())
        };

        rows.push(Row {
            name:     name.clone(),
            model:    if model.is_empty()    { "—".to_string() } else { model },
            serial:   if serial.is_empty()   { "—".to_string() } else { serial },
            firmware: if firmware.is_empty() { "—".to_string() } else { firmware },
            capacity: if capacity.is_empty() { "—".to_string() } else { capacity },
        });
    }

    if rows.is_empty() {
        println!("No block devices found.");
        return Ok(());
    }

    println!("{:<8}  {:<35}  {:<20}  {:<8}  {}", "Device", "Model", "Serial", "Firmware", "Capacity");
    println!("{}", "─".repeat(90));
    for r in &rows {
        println!("{:<8}  {:<35}  {:<20}  {:<8}  {}", r.name, r.model, r.serial, r.firmware, r.capacity);
    }
    Ok(())
}

fn run_growfs(device: &str) -> Result<()> {
    let name     = device.trim_start_matches("/dev/");
    let dev_path = format!("/dev/{}", name);

    // Detect FS type
    let blkid = std::process::Command::new("blkid")
        .args(["-o", "value", "-s", "TYPE", &dev_path])
        .output()
        .map_err(|e| anyhow::anyhow!("blkid failed: {}", e))?;
    let fstype = String::from_utf8_lossy(&blkid.stdout).trim().to_string();

    // Find the mount point (if mounted)
    let mounts_text = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let mount_point = mounts_text.lines()
        .find(|l| l.split_whitespace().next() == Some(&dev_path))
        .and_then(|l| l.split_whitespace().nth(1))
        .map(|s| s.to_string());

    println!("Growing filesystem on {} (type: {})…", dev_path, if fstype.is_empty() { "unknown" } else { &fstype });
    if let Some(ref mp) = mount_point {
        println!("  Mount point: {}", mp);
    }
    println!();

    let status = match fstype.as_str() {
        "ext2" | "ext3" | "ext4" => {
            // resize2fs works on device directly (may need e2fsck -f first if unmounted)
            println!("  Running: resize2fs {}", dev_path);
            std::process::Command::new("resize2fs")
                .arg(&dev_path)
                .status()
                .map_err(|e| anyhow::anyhow!("resize2fs not found: {}", e))?
        }
        "xfs" => {
            let target = mount_point.as_deref().unwrap_or(&dev_path);
            println!("  Running: xfs_growfs {}", target);
            std::process::Command::new("xfs_growfs")
                .arg(target)
                .status()
                .map_err(|e| anyhow::anyhow!("xfs_growfs not found: {}", e))?
        }
        "btrfs" => {
            let target = mount_point.as_deref().unwrap_or(&dev_path);
            println!("  Running: btrfs filesystem resize max {}", target);
            std::process::Command::new("btrfs")
                .args(["filesystem", "resize", "max", target])
                .status()
                .map_err(|e| anyhow::anyhow!("btrfs not found: {}", e))?
        }
        "" => anyhow::bail!("Could not detect filesystem type on {} — is it formatted?", dev_path),
        other => anyhow::bail!("Unsupported filesystem '{}' for online grow (try resize2fs/xfs_growfs/btrfs manually)", other),
    };

    if status.success() {
        println!("\nFilesystem grown successfully.");
    } else {
        anyhow::bail!("Grow command exited with status {}", status);
    }
    Ok(())
}

fn run_scrub(device: Option<&str>) -> Result<()> {
    let mut found_any = false;

    // ── ZFS pools ────────────────────────────────────────────────────
    let zfs_out = std::process::Command::new("zpool")
        .args(["status", "-v"])
        .output();
    if let Ok(out) = zfs_out {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut in_target = false;
        let mut pool_name = String::new();
        for line in text.lines() {
            if line.trim_start().starts_with("pool:") {
                pool_name = line.split_whitespace().nth(1).unwrap_or("").to_string();
                in_target = device.map_or(true, |d| pool_name == d || format!("/dev/{}", d) == pool_name);
            }
            if in_target && line.contains("scan:") {
                found_any = true;
                println!("ZFS pool '{}': {}", pool_name, line.split_once(':').map_or("", |(_, v)| v.trim()));
            }
        }
    }

    // ── BTRFS filesystems ────────────────────────────────────────────
    let mounts_text = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let btrfs_mounts: Vec<(&str, &str)> = mounts_text.lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let dev = parts.next()?;
            let mp  = parts.next()?;
            let fs  = parts.next()?;
            if fs == "btrfs" { Some((dev, mp)) } else { None }
        })
        .collect();

    // Deduplicate by mount point (btrfs can appear multiple times)
    let mut seen_mp: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (dev, mp) in &btrfs_mounts {
        if !seen_mp.insert(mp) { continue; }
        let dev_name = dev.trim_start_matches("/dev/");
        if device.map_or(false, |d| d != dev_name && d != *dev) { continue; }
        found_any = true;

        let status_out = std::process::Command::new("btrfs")
            .args(["scrub", "status", mp])
            .output();
        match status_out {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout);
                let status_line = text.lines()
                    .find(|l| l.contains("Status:") || l.contains("no scrub"))
                    .map(|l| l.trim())
                    .unwrap_or("no status");
                println!("BTRFS {} ({}): {}", mp, dev, status_line);
                // Start scrub if not running
                if text.contains("no scrub") || text.contains("Status: finished") || text.contains("Status: aborted") {
                    println!("  Starting btrfs scrub on {}…", mp);
                    let _ = std::process::Command::new("btrfs")
                        .args(["scrub", "start", mp])
                        .status();
                }
            }
            Err(_) => println!("  btrfs not installed or scrub not available."),
        }
    }

    // ── MD-RAID ──────────────────────────────────────────────────────
    let mdstat = std::fs::read_to_string("/proc/mdstat").unwrap_or_default();
    for line in mdstat.lines() {
        if !line.starts_with("md") { continue; }
        let md_name = line.split_whitespace().next().unwrap_or("").to_string();
        if device.map_or(false, |d| d != md_name && format!("/dev/{}", d) != format!("/dev/{}", md_name)) { continue; }
        found_any = true;

        // Read sync_action
        let sync_path  = format!("/sys/block/{}/md/sync_action", md_name);
        let mismatch_p = format!("/sys/block/{}/md/mismatch_cnt", md_name);
        let sync_action = std::fs::read_to_string(&sync_path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let mismatch = std::fs::read_to_string(&mismatch_p)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "0".to_string());

        if sync_action == "idle" {
            // Start a check
            println!("MD-RAID /dev/{}: idle — starting check scrub…", md_name);
            let _ = std::fs::write(&sync_path, "check");
            println!("  Written 'check' to {}", sync_path);
        } else {
            println!("MD-RAID /dev/{}: sync_action={}, mismatch_cnt={}", md_name, sync_action, mismatch);
        }
    }

    if !found_any {
        if let Some(d) = device {
            println!("No scrub-capable volume found matching '{}'.", d);
            println!("Supported: ZFS pools, BTRFS filesystems, MD-RAID arrays (/dev/mdN).");
        } else {
            println!("No ZFS pools, BTRFS filesystems, or MD-RAID arrays detected.");
        }
    }
    Ok(())
}

fn run_redundancy() -> Result<()> {
    println!("{:<14}  {:<12}  {:<20}  {}", "Device", "Redundancy", "Array/Pool", "State");
    println!("{}", "─".repeat(72));

    // Collect all block devices from sysfs
    let mut all_devs: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/sys/block") {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") { continue; }
            all_devs.push(name);
        }
    }
    all_devs.sort();

    // Build map: device → (array_name, level, state)
    let mut raid_members: std::collections::HashMap<String, (String, String, String)> = std::collections::HashMap::new();

    // MD-RAID
    let mdstat = std::fs::read_to_string("/proc/mdstat").unwrap_or_default();
    let mut current_md = String::new();
    let mut current_level = String::new();
    for line in mdstat.lines() {
        if line.starts_with("md") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            current_md    = parts[0].to_string();
            current_level = parts.get(3).copied().unwrap_or("?").to_string();
        }
        if line.trim_start().starts_with('[') { continue; }
        // member devices appear as "sda[0]" etc.
        for token in line.split_whitespace() {
            if let Some(dev) = token.split('[').next() {
                if all_devs.contains(&dev.to_string()) {
                    let state_path = format!("/sys/block/{}/md/array_state", current_md);
                    let state = std::fs::read_to_string(&state_path)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "?".to_string());
                    raid_members.insert(dev.to_string(), (current_md.clone(), current_level.clone(), state));
                }
            }
        }
    }

    // ZFS — use zpool status output
    let zpool_out = std::process::Command::new("zpool").args(["status"]).output();
    let mut current_pool = String::new();
    let mut current_pool_state = String::new();
    if let Ok(out) = zpool_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pool:") {
                current_pool = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
            }
            if trimmed.starts_with("state:") {
                current_pool_state = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
            }
            // member device lines look like "  sda   ONLINE  ..."
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 && all_devs.contains(&parts[0].to_string()) {
                raid_members.insert(parts[0].to_string(),
                    (current_pool.clone(), "zfs".to_string(), current_pool_state.clone()));
            }
        }
    }

    for dev in &all_devs {
        let (redundancy, array, state) = if let Some((arr, level, st)) = raid_members.get(dev) {
            let red = match level.as_str() {
                "raid1" | "mirror" => "MIRRORED",
                "raid5"            => "RAID-5",
                "raid6"            => "RAID-6",
                "raid10"           => "RAID-10",
                "zfs"              => "ZFS",
                "raid0"            => "RAID-0 (none)",
                _                  => "RAID",
            };
            (red.to_string(), arr.clone(), st.clone())
        } else {
            ("NONE".to_string(), "—".to_string(), "bare".to_string())
        };

        let state_display = match state.as_str() {
            "clean" | "ONLINE" | "active" => format!("✓ {}", state),
            "degraded" | "DEGRADED"       => format!("⚠ {}", state),
            "failed"   | "FAULTED"        => format!("✗ {}", state),
            _                             => state.clone(),
        };

        println!("{:<14}  {:<12}  {:<20}  {}", dev, redundancy, array, state_display);
    }
    Ok(())
}

fn run_trim_report() -> Result<()> {
    // Gather block devices from sysfs
    let mut devs: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/sys/block") {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") { continue; }
            devs.push(name);
        }
    }
    devs.sort();

    let mounts_text = std::fs::read_to_string("/proc/mounts").unwrap_or_default();

    println!("{:<10}  {:<8}  {:<10}  {:<10}  {:<10}  Notes",
             "Device", "Rotational", "TRIM Supp", "Discard", "Last fstrim");
    println!("{}", "─".repeat(74));

    for dev in &devs {
        // Skip partitions (contain a digit after letters, e.g. sda1, nvme0n1p1)
        if dev.chars().last().map_or(false, |c| c.is_ascii_digit()) { continue; }

        let sysfs = format!("/sys/block/{}", dev);
        let rotational = std::fs::read_to_string(format!("{}/queue/rotational", sysfs))
            .map(|s| s.trim() == "1")
            .unwrap_or(false);

        // Only report on SSDs/NVMe
        if rotational { continue; }

        // TRIM support: discard_max_bytes > 0
        let discard_max: u64 = std::fs::read_to_string(format!("{}/queue/discard_max_bytes", sysfs))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let trim_supported = discard_max > 0;

        // Check if any mount uses discard option
        let dev_path = format!("/dev/{}", dev);
        let discard_mount = mounts_text.lines().any(|l| {
            let mut p = l.split_whitespace();
            let d = p.next().unwrap_or("");
            let _mp = p.next();
            let _fs = p.next();
            let opts = p.next().unwrap_or("");
            (d == dev_path || d.starts_with(&format!("{}/", dev_path)) || d.starts_with(&format!("{}p", dev_path)))
                && opts.split(',').any(|o| o == "discard")
        });

        // fstrim last run — check systemd journal for fstrim entries (best effort)
        let fstrim_last = {
            let out = std::process::Command::new("journalctl")
                .args(["-u", "fstrim.service", "--no-pager", "-n", "1", "--output=short"])
                .output();
            match out {
                Ok(o) if !o.stdout.is_empty() => {
                    let text = String::from_utf8_lossy(&o.stdout);
                    text.lines()
                        .last()
                        .map(|l| {
                            l.split_whitespace()
                                .take(3)
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .unwrap_or_else(|| "see journal".to_string())
                }
                _ => "unknown".to_string(),
            }
        };

        let trim_str    = if trim_supported { "yes" } else { "no" };
        let discard_str = if discard_mount  { "mount opt" } else { "—" };
        let rot_str     = "SSD/NVMe";
        let note        = if !trim_supported { "no TRIM support" }
                          else if discard_mount { "continuous discard" }
                          else { "run fstrim periodically" };

        println!("{:<10}  {:<8}  {:<10}  {:<10}  {:<10}  {}",
                 dev, rot_str, trim_str, discard_str,
                 if fstrim_last.len() > 10 { &fstrim_last[..10] } else { &fstrim_last },
                 note);
    }
    Ok(())
}

fn run_io_pressure() -> Result<()> {
    // ── System PSI ───────────────────────────────────────────────────
    println!("System I/O Pressure (PSI)\n");

    let psi_text = std::fs::read_to_string("/proc/pressure/io")
        .unwrap_or_else(|_| "(PSI not available on this kernel — requires Linux 4.20+)".to_string());

    for line in psi_text.lines() {
        // Format: "some avg10=0.00 avg60=0.00 avg300=0.00 total=0"
        let kind = if line.starts_with("some") { "Some (any task stalled)" }
                   else if line.starts_with("full") { "Full (all tasks stalled)" }
                   else { line };
        let stats: Vec<(&str, &str)> = line.split_whitespace()
            .skip(1)
            .filter_map(|kv| kv.split_once('='))
            .collect();
        if stats.is_empty() {
            println!("  {}", line);
            continue;
        }
        println!("  {}:", kind);
        for (k, v) in &stats {
            println!("    {:12} {}", k, v);
        }
        println!();
    }

    // ── Per-device I/O wait from diskstats ───────────────────────────
    println!("Per-Device I/O Wait (from /proc/diskstats)\n");
    println!("{:<12}  {:>12}  {:>12}  {:>14}  {:>14}",
             "Device", "Read ops", "Write ops", "Read ms", "Write ms");
    println!("{}", "─".repeat(70));

    let diskstats = std::fs::read_to_string("/proc/diskstats").unwrap_or_default();
    let mut rows: Vec<(String, u64, u64, u64, u64)> = Vec::new();

    for line in diskstats.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 14 { continue; }
        let name = f[2];
        // Skip partitions (end in digit after letters)
        if name.chars().last().map_or(false, |c| c.is_ascii_digit()) { continue; }
        if name.starts_with("loop") || name.starts_with("ram") { continue; }

        let read_ops:  u64 = f[3].parse().unwrap_or(0);
        let read_ms:   u64 = f[6].parse().unwrap_or(0);
        let write_ops: u64 = f[7].parse().unwrap_or(0);
        let write_ms:  u64 = f[10].parse().unwrap_or(0);

        if read_ops == 0 && write_ops == 0 { continue; }
        rows.push((name.to_string(), read_ops, write_ops, read_ms, write_ms));
    }

    // Sort by total I/O time descending
    rows.sort_by(|a, b| (b.3 + b.4).cmp(&(a.3 + a.4)));

    for (name, rops, wops, rms, wms) in &rows {
        println!("{:<12}  {:>12}  {:>12}  {:>12}ms  {:>12}ms",
                 name, rops, wops, rms, wms);
    }
    Ok(())
}
