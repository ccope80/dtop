use anyhow::Result;
use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicesConfig {
    /// Glob-style patterns of devices to exclude (e.g. "loop*", "sr*")
    pub exclude: Vec<String>,
}

// ── Defaults ─────────────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            alerts:  AlertConfig::default(),
            devices: DevicesConfig::default(),
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
        Self { thresholds: AlertThresholds::default() }
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
        }
    }
}

impl Default for DevicesConfig {
    fn default() -> Self {
        Self {
            exclude: vec!["loop*".into(), "sr*".into(), "ram*".into(), "fd*".into()],
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
