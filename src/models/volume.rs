/// One software RAID array from /proc/mdstat.
#[derive(Debug, Clone)]
pub struct RaidArray {
    pub name:           String,
    pub state:          String,   // "active", "inactive", ...
    pub level:          String,   // "raid1", "raid5", ...
    pub members:        Vec<String>,
    pub capacity_bytes: u64,
    pub bitmap:         String,   // e.g. "[4/4] [UUUU]"
    pub degraded:       bool,
    pub rebuild_pct:    Option<f64>,
}

// ── LVM ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LvmVg {
    pub name:       String,
    pub size_bytes: u64,
    pub free_bytes: u64,
    pub pv_count:   u32,
    pub lv_count:   u32,
}

impl LvmVg {
    pub fn used_bytes(&self) -> u64 { self.size_bytes.saturating_sub(self.free_bytes) }
    pub fn use_pct(&self) -> f64 {
        if self.size_bytes == 0 { return 0.0; }
        self.used_bytes() as f64 / self.size_bytes as f64 * 100.0
    }
}

#[derive(Debug, Clone)]
pub struct LvmLv {
    pub name:       String,
    pub vg_name:    String,
    pub size_bytes: u64,
    #[allow(dead_code)]
    pub attr:       String,
    pub path:       String,
}

#[derive(Debug, Clone)]
pub struct LvmPv {
    pub name:       String,
    pub vg_name:    String,
    pub size_bytes: u64,
    #[allow(dead_code)]
    pub free_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct LvmState {
    pub vgs: Vec<LvmVg>,
    pub lvs: Vec<LvmLv>,
    pub pvs: Vec<LvmPv>,
}

// ── ZFS ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ZfsPool {
    pub name:         String,
    pub size_bytes:   u64,
    pub alloc_bytes:  u64,
    pub free_bytes:   u64,
    pub health:       String,   // "ONLINE", "DEGRADED", "FAULTED", ...
    pub scrub_status: String,   // e.g. "ok (2026-02-09)", "scrubbing 66.7%", "no scrub"
}

impl ZfsPool {
    pub fn use_pct(&self) -> f64 {
        if self.size_bytes == 0 { return 0.0; }
        self.alloc_bytes as f64 / self.size_bytes as f64 * 100.0
    }

    pub fn is_healthy(&self) -> bool { self.health == "ONLINE" }
}
