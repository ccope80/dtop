#[derive(Debug, Clone, PartialEq)]
pub enum SmartStatus {
    Unknown,
    Passed,
    Warning,  // pre-fail attributes near threshold
    Failed,
}

impl SmartStatus {
    pub fn label(&self) -> &'static str {
        match self {
            SmartStatus::Unknown => "    ?",
            SmartStatus::Passed  => " PASS",
            SmartStatus::Warning => " WARN",
            SmartStatus::Failed  => " FAIL",
        }
    }
}

/// One ATA SMART attribute row.
#[derive(Debug, Clone)]
pub struct SmartAttribute {
    pub id:          u32,
    pub name:        String,
    pub value:       u16,
    pub worst:       u16,
    pub thresh:      u16,
    pub prefail:     bool,
    pub raw_value:   u64,
    pub raw_str:     String,
    pub when_failed: String,
}

impl SmartAttribute {
    /// True if this pre-fail attribute is within 10 points of its threshold.
    pub fn is_at_risk(&self) -> bool {
        self.prefail && self.thresh > 0 && self.value <= self.thresh.saturating_add(10)
    }
}

/// NVMe SMART / Health Information Log.
#[derive(Debug, Clone)]
pub struct NvmeHealth {
    pub critical_warning:           u8,
    pub temperature_celsius:        i32,
    pub available_spare_pct:        u8,
    pub available_spare_threshold:  u8,
    pub percentage_used:            u8,
    pub data_units_read:            u64,   // units of 1000 * 512 bytes
    pub data_units_written:         u64,
    pub power_on_hours:             u64,
    pub unsafe_shutdowns:           u64,
    pub media_errors:               u64,
    pub error_log_entries:          u64,
}

impl NvmeHealth {
    /// Approximate bytes read (1 unit = 512 KB).
    pub fn bytes_read(&self) -> u64    { self.data_units_read    * 512 * 1000 }
    pub fn bytes_written(&self) -> u64 { self.data_units_written * 512 * 1000 }
}

/// Complete SMART snapshot for one device.
#[derive(Debug, Clone)]
pub struct SmartData {
    pub status:         SmartStatus,
    pub temperature:    Option<i32>,
    pub power_on_hours: Option<u64>,
    /// ATA SMART attributes (HDD / SATA SSD).
    pub attributes:     Vec<SmartAttribute>,
    /// NVMe-specific health log (NVMe only).
    pub nvme:           Option<NvmeHealth>,
}

impl SmartData {
    /// Derive SmartStatus from parsed data (may downgrade from Passed → Warning).
    pub fn derive_status(&mut self) {
        if self.status == SmartStatus::Failed { return; }

        // Check pre-fail attributes
        for attr in &self.attributes {
            if attr.is_at_risk() {
                self.status = SmartStatus::Warning;
                return;
            }
        }

        // Check NVMe critical warning
        if let Some(nvme) = &self.nvme {
            if nvme.critical_warning != 0 || nvme.media_errors > 0 {
                self.status = SmartStatus::Warning;
                return;
            }
            if nvme.available_spare_pct < nvme.available_spare_threshold {
                self.status = SmartStatus::Warning;
                return;
            }
        }
    }
}
