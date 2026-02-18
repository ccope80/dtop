# dtop

A `btop`-style disk health monitor for Linux sysadmins.

Real-time TUI dashboard for block device I/O, SMART health, filesystem utilisation, NFS latency, software RAID, LVM, and ZFS — plus a headless daemon mode for alert logging and webhook notifications.

![dtop screenshot placeholder](https://via.placeholder.com/800x400?text=dtop+TUI+screenshot)

---

## Features

| Category | Details |
|---|---|
| **Device I/O** | Per-device read/write throughput, IOPS, I/O utilisation bar, live sparklines |
| **SMART health** | Temperature, health status dot, endurance (NVMe % used, HDD power-on hours), attribute table |
| **Filesystem** | Usage bars for all mounted filesystems, sorted by usage |
| **Volume managers** | Software RAID (`/proc/mdstat`), LVM VG/LV/PV, ZFS pools with scrub status |
| **NFS view** | Per-mount latency (read/write RTT ms), op counts, server bytes transferred |
| **Process I/O** | Top processes by read+write, sortable |
| **Alerts** | Configurable thresholds → TUI alert panel, append-only log file, Slack/Discord webhook |
| **Benchmarking** | Quick read benchmark (`dd iflag=direct`) on any selected device |
| **SMART self-test** | Schedule a short SMART test from within the TUI |
| **Health report** | `--report` flag prints a human-readable text summary and exits |
| **JSON snapshot** | `--json` flag prints a full structured JSON snapshot and exits |
| **Daemon mode** | `--daemon` — headless, no TUI, polls and fires alerts; ideal for systemd |
| **Themes** | Default, Dracula, Gruvbox, Nord |
| **Device filter** | `f` key cycles All / NVMe / SSD / HDD — non-matching devices are dimmed |
| **SMART cache** | Persisted across restarts at `~/.local/share/dtop/smart_cache.json` |

---

## Requirements

- Linux (kernel 4.x+)
- `lsblk` (util-linux) — always required
- `smartctl` (smartmontools) — optional, for SMART data
- `zpool` (zfsutils-linux) — optional, for ZFS
- `lvm2` — optional, for LVM
- Root or `disk` group membership for SMART access

---

## Build

```bash
# Release build (recommended)
cargo build --release
sudo cp target/release/dtop /usr/local/bin/

# Debug build
cargo build
```

Minimum Rust: **1.75** (edition 2021).

---

## Usage

```
dtop [OPTIONS]

Options:
  -i, --interval <MS>   Update interval in milliseconds [default: 2000]
      --no-smart        Disable SMART data collection
  -t, --theme <NAME>    Color theme: default, dracula, gruvbox, nord [default: default]
      --json            Print a JSON snapshot of all disk data and exit
      --report          Print a human-readable health report and exit
      --daemon          Run as a headless daemon (no TUI)
  -h, --help            Print help
  -V, --version         Print version
```

### TUI mode

```bash
dtop                        # default 2-second refresh
dtop -i 5000                # 5-second refresh
dtop --no-smart             # skip SMART (faster startup, no root needed)
dtop -t dracula             # Dracula color scheme
```

### One-shot modes

```bash
dtop --json | jq .devices   # JSON snapshot, pipe to jq
dtop --report               # human-readable health summary
```

### Daemon mode

```bash
dtop --daemon --interval 30000   # poll every 30 s, log alerts, fire webhooks
```

---

## Keybindings

### Global

| Key | Action |
|---|---|
| `q` / `Ctrl-C` | Quit |
| `Tab` / `Shift-Tab` | Cycle focus between panels |
| `↑` `↓` / `k` `j` | Navigate list |
| `Enter` / `l` | Drill into device detail |
| `Esc` / `h` | Back / close detail |
| `?` / `F1` | Show help overlay |
| `t` | Cycle color theme |
| `p` | Cycle layout preset (Full / IO-Focus / Storage) |
| `f` | Cycle device filter (All / NVMe / SSD / HDD) |

### Dashboard

| Key | Action |
|---|---|
| `F2` | Process I/O view |
| `F3` | Filesystem overview |
| `F4` | Volume manager (RAID/LVM/ZFS) |
| `F5` | NFS / network mount latency |
| `s` | Cycle sort column (process view) |

### Device detail view

| Key | Action |
|---|---|
| `w` | Cycle history window (60 s / 5 min / 1 h) |
| `b` | Run quick read benchmark on selected device |
| `x` | Schedule SMART short self-test |

---

## Configuration

Config file: `~/.config/dtop/dtop.toml` (created with defaults on first run).

```toml
[devices]
# Glob patterns for devices to exclude from all views
exclude = ["loop*", "ram*"]

[alerts.thresholds]
temp_warn_hdd = 50       # °C
temp_crit_hdd = 60
temp_warn_ssd = 55
temp_crit_ssd = 70
util_warn_pct = 80.0     # I/O utilisation %
util_crit_pct = 95.0
fs_warn_pct   = 85.0     # filesystem usage %
fs_crit_pct   = 95.0
reallocated_warn = 1     # SMART reallocated sectors
pending_warn     = 1
latency_warn_ms  = 20.0  # NFS RTT
latency_crit_ms  = 100.0

[notifications]
webhook_url    = ""      # Slack/Discord incoming webhook URL
notify_warning = false   # Send webhooks for warnings (not just criticals)
```

---

## Alert log

Alerts are appended to `~/.local/share/dtop/alerts.log` (daemon and TUI modes both write here when new alerts fire).

---

## Webhook notifications

Set `webhook_url` in `dtop.toml` to a Slack or Discord incoming webhook. On new alerts dtop fires a POST with a JSON payload compatible with both services.

---

## Daemon / systemd setup

A template unit file is provided at [`contrib/dtop.service`](contrib/dtop.service).

```bash
sudo cp target/release/dtop /usr/local/bin/
sudo cp contrib/dtop.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now dtop
journalctl -u dtop -f
```

Edit the unit file to adjust `--interval`, user/group, and `XDG_*` paths as needed.

---

## Data persistence

| File | Purpose |
|---|---|
| `~/.local/share/dtop/smart_cache.json` | SMART data cache (survives restarts) |
| `~/.local/share/dtop/alerts.log` | Append-only alert history |
| `~/.config/dtop/dtop.toml` | Configuration |

---

## License

MIT — see [LICENSE](LICENSE).
