use crate::config::AlertThresholds;
use crate::models::device::BlockDevice;
use crate::models::filesystem::Filesystem;
use crate::models::smart::SmartStatus;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn label(&self) -> &'static str {
        match self {
            Severity::Info     => "INFO",
            Severity::Warning  => "WARN",
            Severity::Critical => "CRIT",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub severity: Severity,
    pub device:   Option<String>,
    pub mount:    Option<String>,
    pub message:  String,
}

impl Alert {
    pub fn prefix(&self) -> String {
        if let Some(d) = &self.device { format!("[{}] ", d) }
        else if let Some(m) = &self.mount  { format!("[{}] ", m) }
        else { String::new() }
    }

    /// Stable string key identifying this alert condition (for ack/cooldown).
    pub fn key(&self) -> String {
        format!("{}{}{}", self.severity.label(), self.prefix(), self.message)
    }
}

/// Evaluate all alert conditions against current state.
/// Returns a freshly built list sorted Critical → Warning → Info.
pub fn evaluate(devices: &[BlockDevice], filesystems: &[Filesystem], thr: &AlertThresholds) -> Vec<Alert> {
    let mut alerts: Vec<Alert> = Vec::new();

    for dev in devices {
        // ── SMART / temperature ───────────────────────────────────────
        if let Some(smart) = &dev.smart {
            // Overall health
            if smart.status == SmartStatus::Failed {
                alerts.push(Alert {
                    severity: Severity::Critical,
                    device:   Some(dev.name.clone()),
                    mount:    None,
                    message:  "SMART health check FAILED".into(),
                });
            }

            // Temperature thresholds (config-driven)
            if let Some(temp) = smart.temperature {
                let (warn, crit) = if dev.rotational {
                    (thr.temperature_warn_hdd, thr.temperature_crit_hdd)
                } else {
                    (thr.temperature_warn_ssd, thr.temperature_crit_ssd)
                };
                if temp >= crit {
                    alerts.push(Alert {
                        severity: Severity::Critical,
                        device:   Some(dev.name.clone()),
                        mount:    None,
                        message:  format!("Temperature {}°C ≥ critical threshold {}°C", temp, crit),
                    });
                } else if temp >= warn {
                    alerts.push(Alert {
                        severity: Severity::Warning,
                        device:   Some(dev.name.clone()),
                        mount:    None,
                        message:  format!("Temperature {}°C ≥ warning threshold {}°C", temp, warn),
                    });
                }
            }

            // Pre-fail attributes at risk
            for attr in &smart.attributes {
                if attr.is_at_risk() {
                    alerts.push(Alert {
                        severity: Severity::Warning,
                        device:   Some(dev.name.clone()),
                        mount:    None,
                        message:  format!(
                            "Pre-fail attr {} value {} near threshold {}",
                            attr.name, attr.value, attr.thresh
                        ),
                    });
                }
            }

            // Pending sectors (ID 197)
            let pending = smart.attributes.iter()
                .find(|a| a.id == 197)
                .map(|a| a.raw_value)
                .unwrap_or(0);
            if pending > 0 {
                alerts.push(Alert {
                    severity: Severity::Warning,
                    device:   Some(dev.name.clone()),
                    mount:    None,
                    message:  format!("{} pending sector(s) detected", pending),
                });
            }

            // Reallocated sectors (ID 5)
            let realloc = smart.attributes.iter()
                .find(|a| a.id == 5)
                .map(|a| a.raw_value)
                .unwrap_or(0);
            if realloc > 0 {
                alerts.push(Alert {
                    severity: Severity::Warning,
                    device:   Some(dev.name.clone()),
                    mount:    None,
                    message:  format!("{} reallocated sector(s)", realloc),
                });
            }

            // Pre-fail attribute degradation since last poll
            if let Some(prev_smart) = &dev.smart_prev {
                for curr_attr in &smart.attributes {
                    if !curr_attr.prefail { continue; }
                    if let Some(prev_attr) = prev_smart.attributes.iter().find(|a| a.id == curr_attr.id) {
                        if curr_attr.value < prev_attr.value {
                            alerts.push(Alert {
                                severity: Severity::Warning,
                                device:   Some(dev.name.clone()),
                                mount:    None,
                                message:  format!(
                                    "Pre-fail attr {} degraded {} → {} (↓{})",
                                    curr_attr.name, prev_attr.value, curr_attr.value,
                                    prev_attr.value - curr_attr.value
                                ),
                            });
                        }
                    }
                }
            }

            // NVMe-specific
            if let Some(nvme) = &smart.nvme {
                if nvme.media_errors > 0 {
                    alerts.push(Alert {
                        severity: Severity::Warning,
                        device:   Some(dev.name.clone()),
                        mount:    None,
                        message:  format!("{} uncorrectable media error(s)", nvme.media_errors),
                    });
                }
                if nvme.available_spare_pct < nvme.available_spare_threshold {
                    alerts.push(Alert {
                        severity: Severity::Warning,
                        device:   Some(dev.name.clone()),
                        mount:    None,
                        message:  format!(
                            "NVMe spare {}% below threshold {}%",
                            nvme.available_spare_pct, nvme.available_spare_threshold
                        ),
                    });
                }
                if nvme.critical_warning != 0 {
                    alerts.push(Alert {
                        severity: Severity::Critical,
                        device:   Some(dev.name.clone()),
                        mount:    None,
                        message:  format!("NVMe critical warning byte: 0x{:02X}", nvme.critical_warning),
                    });
                }
            }
        }

        // ── I/O utilisation sustained ─────────────────────────────────
        if dev.io_util_pct >= thr.io_util_warn_pct {
            alerts.push(Alert {
                severity: Severity::Warning,
                device:   Some(dev.name.clone()),
                mount:    None,
                message:  format!("I/O utilisation {:.0}% (sustained)", dev.io_util_pct),
            });
        }

        // ── I/O latency ───────────────────────────────────────────────
        let lat = dev.avg_read_latency_ms.max(dev.avg_write_latency_ms);
        if thr.latency_crit_ms > 0.0 && lat >= thr.latency_crit_ms {
            alerts.push(Alert {
                severity: Severity::Critical,
                device:   Some(dev.name.clone()),
                mount:    None,
                message:  format!("I/O latency {:.0}ms ≥ critical threshold {:.0}ms", lat, thr.latency_crit_ms),
            });
        } else if thr.latency_warn_ms > 0.0 && lat >= thr.latency_warn_ms {
            alerts.push(Alert {
                severity: Severity::Warning,
                device:   Some(dev.name.clone()),
                mount:    None,
                message:  format!("I/O latency {:.0}ms ≥ warning threshold {:.0}ms", lat, thr.latency_warn_ms),
            });
        }
    }

    // ── Filesystem thresholds ─────────────────────────────────────────
    for fs in filesystems {
        let pct = fs.use_pct();
        if pct >= thr.filesystem_crit_pct {
            alerts.push(Alert {
                severity: Severity::Critical,
                device:   None,
                mount:    Some(fs.mount.clone()),
                message:  format!("{:.0}% full — critically low space", pct),
            });
        } else if pct >= thr.filesystem_warn_pct {
            alerts.push(Alert {
                severity: Severity::Warning,
                device:   None,
                mount:    Some(fs.mount.clone()),
                message:  format!("{:.0}% full", pct),
            });
        }

        let ipct = fs.inode_pct();
        if ipct >= thr.inode_crit_pct {
            alerts.push(Alert {
                severity: Severity::Critical,
                device:   None,
                mount:    Some(fs.mount.clone()),
                message:  format!("Inodes {:.0}% used — critically low", ipct),
            });
        } else if ipct >= thr.inode_warn_pct {
            alerts.push(Alert {
                severity: Severity::Warning,
                device:   None,
                mount:    Some(fs.mount.clone()),
                message:  format!("Inodes {:.0}% used", ipct),
            });
        }
    }

    // Sort: Critical first, then Warning, then Info
    alerts.sort_by(|a, b| b.severity.cmp(&a.severity));
    alerts
}
