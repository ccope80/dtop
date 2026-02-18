# dtop

A **btop-style disk health monitor** for Linux — interactive TUI + 60+ CLI tools for SMART monitoring, I/O performance, filesystem health, and storage alerts.

![dtop screenshot placeholder](https://via.placeholder.com/800x400?text=dtop+TUI+screenshot)

## Features

- **Full-screen TUI** with live I/O sparklines, health scores, temperature trends, and alert badges
- **SMART monitoring** — attribute tracking, anomaly detection, baseline snapshots, self-test scheduling
- **6 views**: Dashboard · Process I/O · Filesystems · Volume Manager (RAID/LVM/ZFS) · NFS Latency · Alert Log
- **Alert system** — configurable rules, persistent log, webhook notifications, Nagios-compatible exit codes
- **60+ CLI commands** for reporting, diagnostics, and maintenance without launching the TUI
- **Persistence** — health history, write endurance tracking, SMART baselines across sessions

## Installation

### Build from source

```bash
# Prerequisites: Rust toolchain, smartmontools
cargo build --release

# Install to /usr/local/bin (requires root)
sudo ./target/release/dtop --install
```

`--install` copies the release binary to `/usr/local/bin/dtop`, installs the man page to `/usr/local/share/man/man1/dtop.1`, and installs bash completion to `/etc/bash_completion.d/dtop`.

### Dependencies

| Tool | Purpose | Required |
|------|---------|----------|
| `smartctl` | SMART polling | Recommended |
| `hdparm` | HDD power/APM control | Optional |
| `nvme` | NVMe error log | Optional |
| `lsblk` | Block device enumeration | Yes |

Minimum Rust: **1.75** (edition 2021).

## TUI Usage

```
dtop              # launch full TUI
dtop --no-smart   # disable SMART polling (faster startup)
dtop --interval 2000  # 2-second refresh
dtop -t dracula   # Dracula color theme
```

### Key bindings

#### Global

| Key | Action |
|-----|--------|
| `q` / `Ctrl-C` | Quit |
| `?` / `F1` | Help overlay (scrollable) |
| `t` | Cycle color theme |
| `C` | Config overlay |
| `Tab` / `Shift-Tab` | Cycle panels |
| `↑↓` / `j k` | Navigate list |
| `g` / `G` | Jump first / last |

#### Views

| Key | Action |
|-----|--------|
| `F2` | Process I/O view |
| `F3` | Filesystem overview |
| `F4` | RAID / LVM / ZFS volume manager |
| `F5` | NFS mount latency view |
| `F6` | Alert log viewer |

#### Dashboard

| Key | Action |
|-----|--------|
| `f` | Cycle device filter (All/NVMe/SSD/HDD) |
| `s` | Cycle sort order |
| `p` | Cycle layout preset |
| `a` | Acknowledge all alerts (Enter = ack one) |
| `Enter` | Open device detail |

#### Device Detail

| Key | Action |
|-----|--------|
| `w` | Cycle history window (60s/5m/1h) |
| `r` | Force SMART re-poll |
| `B` | Save SMART baseline snapshot |
| `b` | Run sequential read benchmark |
| `x` | Schedule SMART short self-test |

## CLI Examples

```bash
# Health check (Nagios-compatible)
dtop --check

# One-line status
dtop --summary

# Full HTML report
dtop --report-html /tmp/report.html

# Worst devices first
dtop --top-health

# SMART sector errors across all drives
dtop --sector-errors

# Rolling I/O stats
dtop --iostat --count 10

# Filesystem fill forecast
dtop --forecast

# Watch for hotplug events
dtop --hotplug

# Multi-day health trend
dtop --health-trend sda

# Full SMART report for one device
dtop --device-report sda

# Schedule a SMART self-test and wait for completion
dtop --schedule-test sda --wait

# Show SMART anomaly log
dtop --anomalies

# Top processes by disk I/O
dtop --top-io

# Disk capacity inventory
dtop --capacity

# View partition table with UUIDs and FS types
dtop --partition-table sda

# HDD power state
dtop --power-state sda

# I/O pressure stall info
dtop --io-pressure

# Print man page
dtop --man | man -l -
```

## Configuration

Config file: `~/.config/dtop/dtop.toml` (hot-reloaded every 30s)

```bash
dtop --edit-config   # open in $EDITOR
dtop --config        # print current values
```

```toml
[devices]
exclude = ["loop*", "ram*"]

[alerts.thresholds]
temp_warn_hdd    = 50
temp_crit_hdd    = 60
temp_warn_ssd    = 55
temp_crit_ssd    = 70
util_warn_pct    = 80.0
util_crit_pct    = 95.0
fs_warn_pct      = 85.0
fs_crit_pct      = 95.0
reallocated_warn = 1
pending_warn     = 1
latency_warn_ms  = 20.0
latency_crit_ms  = 100.0

[notifications]
webhook_url    = ""
notify_warning = false
```

## Data Files

All persistent data lives in `~/.local/share/dtop/`:

| File | Purpose |
|------|---------|
| `alert.log` | Full alert history |
| `health_history.json` | Per-device health score trends |
| `write_endurance.json` | Cumulative write tracking |
| `smart_baselines/` | SMART baseline snapshots |
| `anomalies.json` | SMART anomaly log |
| `smart_cache.json` | SMART data cache (survives restarts) |

## Man Page

```bash
# View man page without installing
dtop --man | man -l -

# After install:
man dtop
```

## Daemon / systemd

A template unit file is provided at [`contrib/dtop.service`](contrib/dtop.service).

```bash
sudo cp target/release/dtop /usr/local/bin/
sudo cp contrib/dtop.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now dtop
journalctl -u dtop -f
```

## License

MIT — see [LICENSE](LICENSE).
