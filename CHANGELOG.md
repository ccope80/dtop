# Changelog

All notable changes to dtop are documented here.

## [Unreleased]

## [0.1.0] — 2026-02-18

### Phase 48 Changes
- `--iostat` now uses `--interval` (ms) as the poll cadence instead of a fixed 1-second delay
- `--iostat` `--count` default changed from 10 to 0 (continuous output until Ctrl-C)
- Mouse click device selection in the device list panel verified and working correctly

### Core TUI
- Full-screen interactive dashboard with device list, health scores, I/O sparklines, temperature
- Device detail panel: SMART attributes, temp history sparkline, benchmark, self-test, baseline diff
- Tab indicator in detail panel (SMART / I/O / Temp sections)
- 6 views: Dashboard, Process I/O (F2), Filesystem (F3), Volume Manager (F4), NFS Latency (F5), Alert Log (F6)
- Scrollable help overlay (? / F1) with all keybindings
- Scrollable config overlay (C key)
- Scrollable alert log with text search (/ key) and severity filter (s key)
- Per-alert acknowledge with Enter; bulk ack with a
- Device list scrollbar when devices exceed visible rows
- Alert badge (▲Ncrit / ▲Nwarn) in device list panel title
- Fleet health bar in dashboard header (proportional ok/warn/crit segments)
- Context-sensitive footer hints per active view
- Tab cycling through 5 dashboard panels
- Mouse support: click to select device in device list, scroll to navigate, double-click to open detail
- g/G jump to first/last in all views
- 4 color themes (cycle with t)
- Config hot-reload every 30s with flash indicator

### SMART & Health
- Background SMART polling with configurable interval
- Health score (0–100) from SMART attributes + temperature + age
- SMART anomaly detection and persistent anomaly log
- SMART baseline snapshots (B key / --save-baseline)
- Baseline diff shown in detail panel
- Health score history persisted across sessions (5-min intervals, 7.5 days)
- Write endurance tracking (cumulative bytes written)
- Self-test scheduling (short/long, with --wait polling)
- Self-test log display in detail panel and --device-report

### Alerts
- Configurable alert rules (threshold, age, SMART status, temperature)
- Alert history ring (in-memory) + persistent log file
- Alert cooldown to suppress re-fires
- Desktop notifications (notify-send)
- Webhook notifications
- Alert log viewer with full history, search, and filter
- Weekly digest (--daemon mode)

### CLI Reporting
- --check (Nagios exit codes), --summary, --watch N
- --report (human-readable), --report-html, --report-md
- --json, --csv, --diff (snapshot comparison)
- --device-report DEV (full SMART + self-test log)
- --alerts [--since AGE], --anomalies, --baselines, --endurance
- --top-io, --top-temp, --top-health
- --health-history DEV [--days N], --health-trend [DEV]
- --forecast (filesystem fill-rate + ETA)
- --iostat [DEV] [--count N] (respects --interval for poll cadence; --count defaults to 0 for continuous output)
- --capacity, --disk-info DEV, --disk-model [DEV], --disk-temps
- --smart-attr DEV ATTR, --smart-errors DEV, --sector-errors [DEV]
- --partition-table DEV, --blkid, --mount, --lsof DEV|MOUNT
- --dmesg [DEV], --verify DEV, --bench DEV
- --cumulative-io [DEV], --io-pressure, --cache-stats
- --queue-depth [DEV], --write-barrier [DEV]
- --power-state [DEV], --redundancy, --trim-report
- --scrub [DEV], --du [PATH]

### Maintenance CLI
- --trim [MOUNT], --growfs DEV, --label DEV[=LABEL]
- --spindown DEV [--deep], --apm DEV[=LEVEL]
- --schedule-test DEV [--long] [--wait], --save-baseline DEV
- --clear-anomalies [DEV] [--yes], --secure-erase DEV [--yes]
- --io-sched [DEV[=SCHED]], --hotplug
- --print-service, --test-webhook, --edit-config

### Installation & Docs
- --man (groff man page, pipe to man -l -)
- --install (copies binary + man page + completions to /usr/local)
- --completions SHELL (bash/zsh/fish)
- README.md with full documentation
- Shell completion scripts
- Systemd service unit (--print-service)
