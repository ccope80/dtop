/// One mounted filesystem with live usage data.
#[derive(Debug, Clone)]
pub struct Filesystem {
    pub device:      String,
    pub mount:       String,
    pub fs_type:     String,
    pub total_bytes: u64,
    pub used_bytes:  u64,
    pub avail_bytes: u64,
    pub total_inodes: u64,
    pub free_inodes:  u64,

    // Fill rate tracking (computed in App::collect_fast)
    pub fill_rate_bps:   Option<f64>,  // bytes/sec (positive = filling, negative = shrinking)
    pub days_until_full: Option<f64>,  // projected days until avail_bytes == 0
}

impl Filesystem {
    pub fn use_pct(&self) -> f64 {
        if self.total_bytes == 0 { return 0.0; }
        (self.total_bytes - self.avail_bytes) as f64 / self.total_bytes as f64 * 100.0
    }

    pub fn inode_pct(&self) -> f64 {
        if self.total_inodes == 0 { return 0.0; }
        (self.total_inodes - self.free_inodes) as f64 / self.total_inodes as f64 * 100.0
    }

    /// Returns the short device name ("sda1" from "/dev/sda1").
    pub fn short_device(&self) -> &str {
        self.device.trim_start_matches("/dev/").trim_start_matches("mapper/")
    }
}
