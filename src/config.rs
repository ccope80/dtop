use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub alerts: AlertConfig,

    #[serde(default)]
    pub devices: DevicesConfig,

    #[serde(default)]
    pub notifications: NotificationsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Fast tick interval in milliseconds (I/O sampling rate)
    pub update_interval_ms: u64,
    /// SMART refresh interval in seconds
    pub smart_interval_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    #[serde(default)]
    pub thresholds: AlertThresholds,
    /// Suppress re-alerting the same condition for this many hours (0 = no cooldown).
    #[serde(default)]
    pub cooldown_hours: u64,
    /// Per-attribute SMART alert rules evaluated against raw values.
    #[serde(default = "SmartAlertRule::defaults")]
    pub smart_rules: Vec<SmartAlertRule>,
}

/// A configurable SMART attribute alert rule.
///
/// Example in dtop.toml:
/// ```toml
/// [[alerts.smart_rules]]
/// attr     = 5       # Reallocated Sectors
/// op       = "gt"    # gt, gte, lt, lte, eq, ne
/// value    = 0
/// severity = "warn"  # "warn" or "crit"
/// # message = "custom override"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartAlertRule {
    /// SMART attribute ID (e.g. 5=reallocated, 197=pending, 198=uncorrectable)
    pub attr: u32,
    /// Comparison operator applied to the attribute's raw value: "gt", "gte", "lt", "lte", "eq", "ne"
    pub op: String,
    /// Threshold value
    pub value: u64,
    /// "warn" or "crit"
    pub severity: String,
    /// Optional custom message; None = auto-generated from attr name + raw value
    #[serde(default)]
    pub message: Option<String>,
}

impl SmartAlertRule {
    pub fn defaults() -> Vec<Self> {
        vec![
            SmartAlertRule { attr: 5,   op: "gt".into(), value: 0, severity: "warn".into(), message: None },
            SmartAlertRule { attr: 197, op: "gt".into(), value: 0, severity: "warn".into(), message: None },
            SmartAlertRule { attr: 198, op: "gt".into(), value: 0, severity: "crit".into(), message: None },
        ]
    }

    pub fn matches(&self, raw_value: u64) -> bool {
        match self.op.as_str() {
            "gt"  | ">"  => raw_value >  self.value,
            "gte" | ">=" => raw_value >= self.value,
            "lt"  | "<"  => raw_value <  self.value,
            "lte" | "<=" => raw_value <= self.value,
            "eq"  | "==" => raw_value == self.value,
            "ne"  | "!=" => raw_value != self.value,
            _             => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub filesystem_warn_pct:  f64,
    pub filesystem_crit_pct:  f64,
    pub inode_warn_pct:       f64,
    pub inode_crit_pct:       f64,
    pub temperature_warn_ssd: i32,
    pub temperature_crit_ssd: i32,
    pub temperature_warn_hdd: i32,
    pub temperature_crit_hdd: i32,
    pub io_util_warn_pct:     f64,
    /// Average read latency warning threshold (ms). 0 = disabled.
    pub latency_warn_ms:      f64,
    /// Average read latency critical threshold (ms). 0 = disabled.
    pub latency_crit_ms:      f64,
    /// Alert (warning) when a filesystem is projected to fill within this many days. 0 = disabled.
    pub fill_days_warn:       f64,
    /// Alert (critical) when a filesystem is projected to fill within this many days. 0 = disabled.
    pub fill_days_crit:       f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicesConfig {
    /// Glob-style patterns of devices to exclude (e.g. "loop*", "sr*")
    pub exclude: Vec<String>,
    /// Friendly aliases for devices: { "sda" = "boot-ssd", "sdb" = "data-hdd" }
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    /// Slack / Discord / generic webhook URL for alert POSTs. Empty = disabled.
    pub webhook_url: String,
    /// Fire webhook for new Critical-severity alerts.
    pub notify_critical: bool,
    /// Fire webhook for new Warning-severity alerts.
    pub notify_warning: bool,
    /// Send a desktop notification via notify-send when new alerts fire (TUI mode).
    pub notify_send: bool,
}

// ── Defaults ─────────────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        Self {
            general:       GeneralConfig::default(),
            alerts:        AlertConfig::default(),
            devices:       DevicesConfig::default(),
            notifications: NotificationsConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { update_interval_ms: 2000, smart_interval_sec: 300 }
    }
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            thresholds:   AlertThresholds::default(),
            cooldown_hours: 0,
            smart_rules:  SmartAlertRule::defaults(),
        }
    }
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            filesystem_warn_pct:  85.0,
            filesystem_crit_pct:  95.0,
            inode_warn_pct:       85.0,
            inode_crit_pct:       95.0,
            temperature_warn_ssd: 55,
            temperature_crit_ssd: 70,
            temperature_warn_hdd: 50,
            temperature_crit_hdd: 60,
            io_util_warn_pct:     95.0,
            latency_warn_ms:      50.0,
            latency_crit_ms:      200.0,
            fill_days_warn:       14.0,
            fill_days_crit:       3.0,
        }
    }
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            webhook_url:      String::new(),
            notify_critical:  true,
            notify_warning:   false,
            notify_send:      false,
        }
    }
}

impl Default for DevicesConfig {
    fn default() -> Self {
        Self {
            exclude: vec!["loop*".into(), "sr*".into(), "ram*".into(), "fd*".into()],
            aliases: HashMap::new(),
        }
    }
}

// ── Load / Save ───────────────────────────────────────────────────────

impl Config {
    pub fn load() -> Self {
        match try_load() {
            Ok(c)  => c,
            Err(_) => {
                // Write defaults on first run (best-effort)
                let _ = try_write_defaults();
                Config::default()
            }
        }
    }

    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("dtop").join("dtop.toml"))
    }
}

fn try_load() -> Result<Config> {
    let path = Config::config_path().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    let text = fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&text)?;
    Ok(cfg)
}

fn try_write_defaults() -> Result<()> {
    let path = Config::config_path().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(&Config::default())?;
    fs::write(path, format!("# DTop configuration\n# Generated on first run — edit freely\n\n{}", text))?;
    Ok(())
}
