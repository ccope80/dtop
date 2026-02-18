# dtop

A **btop-style disk health monitor** for Linux — interactive TUI + 60+ CLI tools for SMART monitoring, I/O performance, filesystem health, and storage alerts.

```
 DTop v0.1 — Default   !! 1 CRIT  0 WARN                                            21:51:05
 Fleet  R:316.5 KB/s  W:5.9 KB/s   1 devs  1●  [██████████]  avg health: 100/100   up 0m3s
┌1 Devices  (1 total)  ▲1crit────────────────────────────────┐┌2 I/O Throughput──────────────┐
│▶  sda    SSD  100● ▁▁     38°C  ████░░░░  43%  R:12.4 MB/s ││Read  12.4 MB/s               │
│   nvme0  NVMe  98● █████  41°C  ░░░░░░░░   0%  R: 0.0 B/s  ││███████████                   │
│   sdb    HDD   87● ███▁▁  44°C  ░░░░░░░░   0%  R: 0.0 B/s  ││Write 5.9 KB/s                │
└─────────────────────────────────────────────────────────────┘│█▁                            │
┌3 Filesystem Usage───────────────────────────────────────────────────────────────────────────┐
│/          ext4   95.8 GB  used 16.7 GB  avail 79.2 GB  +153.5 MB/day  full ~>1yr            │
│███████████████████████████████                              17%                              │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
┌4 Temperature & SMART──────────────────────┐┌5 Alerts  (1 active)──────────────────────────┐
│  sda    38°C  ████░░░░░░  █▄▃ PASS        ││  CRIT  [sda] SMART health check FAILED        │
│  nvme0  41°C  █████░░░░░  ▅▄█ PASS        ││  ── recent ─────────────────                  │
│  sdb    44°C  █████░░░░░  ▃▄▅ PASS        ││  21:51:01  CRIT  [sda] SMART health check...  │
└───────────────────────────────────────────┘└──────────────────────────────────────────────┘
  q Quit   Tab Focus   ↑↓/jk Select   Enter Detail   s Sort   f Filter   t Theme   ? Help
```

## Features

- **Full-screen TUI** — live I/O sparklines, health scores, temperature trends, per-device alert badges
- **SMART monitoring** — attribute tracking, anomaly detection, baseline snapshots, self-test scheduling
- **6 views** — Dashboard · Process I/O · Filesystems · Volume Manager (RAID/LVM/ZFS) · NFS Latency · Alert Log
- **Alert system** — configurable rules, persistent log, webhook notifications, Nagios-compatible exit codes
- **60+ CLI commands** — reporting, diagnostics, and maintenance without launching the TUI
- **Persistence** — health history, write endurance tracking, SMART baselines survive restarts

## Installation

### Pre-built binary

Download `dtop-linux-x86_64` from the [latest release](https://github.com/ccope80/dtop/releases/latest):

```bash
chmod +x dtop-linux-x86_64
sudo mv dtop-linux-x86_64 /usr/local/bin/dtop
```

### Build from source

```bash
# Prerequisites: Rust toolchain (1.75+), smartmontools
git clone https://github.com/ccope80/dtop
cd dtop
cargo build --release

# Install binary, man page, and bash completions (requires root)
sudo ./target/release/dtop --install
```

### Dependencies

| Tool | Purpose | Required |
|------|---------|----------|
| `smartctl` | SMART polling | Recommended |
| `lsblk` | Block device enumeration | Yes |
| `hdparm` | HDD power/APM control | Optional |
| `nvme` | NVMe error log | Optional |

Run `dtop --diag` to check which tools are available on your system.

## Quick Start

```bash
dtop                    # launch TUI
dtop --summary          # one-line health status
dtop --check            # Nagios-compatible exit code (0/1/2)
dtop --report           # full health report
dtop --top-health       # worst devices first
```

## TUI Keybindings

### Global

| Key | Action |
|-----|--------|
| `q` / `Ctrl-C` | Quit |
| `?` / `F1` | Help overlay (scrollable) |
| `t` | Cycle color theme |
| `C` | Config overlay |
| `Tab` / `Shift-Tab` | Cycle panels |
| `↑↓` / `j k` | Navigate |
| `g` / `G` | Jump first / last |

### Views

| Key | View |
|-----|------|
| `F2` | Process I/O |
| `F3` | Filesystem overview |
| `F4` | RAID / LVM / ZFS volume manager |
| `F5` | NFS mount latency |
| `F6` | Alert log (`/` to search) |

### Dashboard

| Key | Action |
|-----|--------|
| `f` | Cycle device filter (All / NVMe / SSD / HDD) |
| `s` | Cycle sort order |
| `p` | Cycle layout preset |
| `a` | Acknowledge all alerts (`Enter` = ack one) |
| `Enter` | Open device detail |

### Device Detail

| Key | Action |
|-----|--------|
| `w` | Cycle history window (60s / 5m / 1h) |
| `r` | Force SMART re-poll |
| `B` | Save SMART baseline snapshot |
| `b` | Sequential read benchmark |
| `x` | Schedule SMART short self-test |

## CLI Reference

```bash
# Reporting
dtop --report                          # human-readable health report
dtop --report-html --output report.html  # self-contained HTML report
dtop --report-md --output report.md    # Markdown report
dtop --json                            # JSON snapshot
dtop --csv                             # CSV snapshot
dtop --watch 60                        # rolling status every 60s

# SMART
dtop --device-report sda              # full SMART report for one device
dtop --smart-errors sda               # ATA/NVMe error log
dtop --sector-errors                  # pending/reallocated sectors
dtop --anomalies                       # SMART anomaly log
dtop --health-history sda --days 30   # health score history
dtop --health-trend sda               # multi-day ASCII chart
dtop --schedule-test sda --wait       # run self-test and wait
dtop --save-baseline sda              # save SMART baseline

# I/O & Performance
dtop --iostat                          # rolling I/O stats (Ctrl-C to stop)
dtop --iostat sda --count 10          # 10 samples for one device
dtop --top-io                          # top processes by disk I/O
dtop --bench sda                       # sequential read benchmark
dtop --io-pressure                     # PSI I/O pressure stall info

# Filesystem
dtop --forecast                        # fill-rate + ETA per mount
dtop --du /var                         # top directories by usage
dtop --trim                            # run fstrim on all mounts
dtop --trim-report                     # TRIM support per SSD

# Devices
dtop --capacity                        # capacity inventory table
dtop --disk-model                      # model/serial/firmware inventory
dtop --partition-table sda            # partition layout + UUIDs
dtop --power-state sda                # HDD power state
dtop --top-health                      # devices by health score (worst first)
dtop --top-temp                        # devices by temperature

# Alerts
dtop --alerts                          # recent alert log
dtop --alerts --since 7d              # alerts from last 7 days

# Maintenance
dtop --spindown sda                    # HDD standby
dtop --apm sda=127                    # set HDD APM level
dtop --scrub                           # start/check BTRFS/ZFS/MD scrub
dtop --hotplug                         # watch for device add/remove events

# System
dtop --diag                            # self-diagnostic: tools, config, cache
dtop --version-info                    # version + tool availability
dtop --man | man -l -                 # view man page without installing
```

## Configuration

Config file: `~/.config/dtop/dtop.toml` — hot-reloaded every 30 seconds.

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

[notifications]
webhook_url    = ""
notify_warning = false
```

## Daemon / systemd

Run dtop as a headless alert daemon:

```bash
# Print a ready-made systemd unit file
dtop --print-service | sudo tee /etc/systemd/system/dtop.service

sudo systemctl daemon-reload
sudo systemctl enable --now dtop
journalctl -u dtop -f
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
| `smart_cache.json` | SMART data cache |
