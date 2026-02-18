/// Raw cumulative I/O bytes for one process (one snapshot).
#[derive(Debug, Clone)]
pub struct RawProcessIO {
    #[allow(dead_code)]
    pub pid:         u32,
    pub comm:        String,
    pub uid:         u32,
    pub read_bytes:  u64,
    pub write_bytes: u64,
}

/// Per-second I/O rates for one process.
#[derive(Debug, Clone)]
pub struct ProcessIORates {
    pub pid:           u32,
    pub comm:          String,
    pub username:      String,
    pub read_per_sec:  f64,
    pub write_per_sec: f64,
}

impl ProcessIORates {
    pub fn total_per_sec(&self) -> f64 {
        self.read_per_sec + self.write_per_sec
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessSort {
    WritePerSec,
    ReadPerSec,
    Total,
    Pid,
    Name,
}

impl ProcessSort {
    pub fn next(&self) -> Self {
        match self {
            ProcessSort::WritePerSec => ProcessSort::ReadPerSec,
            ProcessSort::ReadPerSec  => ProcessSort::Total,
            ProcessSort::Total       => ProcessSort::Pid,
            ProcessSort::Pid         => ProcessSort::Name,
            ProcessSort::Name        => ProcessSort::WritePerSec,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ProcessSort::WritePerSec => "Write/s",
            ProcessSort::ReadPerSec  => "Read/s",
            ProcessSort::Total       => "Total/s",
            ProcessSort::Pid         => "PID",
            ProcessSort::Name        => "Name",
        }
    }
}
