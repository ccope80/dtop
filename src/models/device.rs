use crate::models::smart::{SmartData, SmartStatus};
use crate::util::ring_buffer::RingBuffer;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceType {
    NVMe,
    SSD,
    HDD,
    Virtual,
    Unknown,
}

impl DeviceType {
    pub fn label(&self) -> &'static str {
        match self {
            DeviceType::NVMe    => "NVMe",
            DeviceType::SSD     => " SSD",
            DeviceType::HDD     => " HDD",
            DeviceType::Virtual => " VRT",
            DeviceType::Unknown => "  ? ",
        }
    }
}

/// A partition/child device as reported by lsblk.
#[derive(Debug, Clone)]
pub struct Partition {
    pub name:       String,
    pub size:       u64,
    pub fs_type:    Option<String>,
    pub mountpoint: Option<String>,
}

/// One block device with live metrics and SMART data.
#[derive(Debug)]
pub struct BlockDevice {
    pub name:           String,
    pub dev_type:       DeviceType,
    pub model:          Option<String>,
    pub serial:         Option<String>,
    pub capacity_bytes: u64,
    pub rotational:     bool,
    pub transport:      Option<String>,
    pub partitions:     Vec<Partition>,

    // Real-time I/O (updated each fast tick)
    pub read_bytes_per_sec:   f64,
    pub write_bytes_per_sec:  f64,
    pub read_iops:            f64,
    pub write_iops:           f64,
    pub io_util_pct:          f64,
    pub avg_read_latency_ms:  f64,   // average ms per read op this tick
    pub avg_write_latency_ms: f64,   // average ms per write op this tick

    // History (KB/s, 1800 samples @ 2 s = 1 h)
    pub read_history:     RingBuffer,
    pub write_history:    RingBuffer,
    pub util_history:     RingBuffer,
    // Latency history (µs*10 stored as u64 for sparkline, to preserve sub-ms detail)
    pub read_lat_history:  RingBuffer,
    pub write_lat_history: RingBuffer,
    // Temperature history (°C, sampled each SMART poll cycle)
    pub temp_history:      RingBuffer,

    // SMART (updated on slow poll / on-demand)
    pub smart:           Option<SmartData>,
    pub smart_prev:      Option<SmartData>,  // previous poll — used for delta arrows
    pub smart_polled_at: Option<Instant>,
}

impl BlockDevice {
    pub fn new(name: String) -> Self {
        Self {
            name,
            dev_type:       DeviceType::Unknown,
            model:          None,
            serial:         None,
            capacity_bytes: 0,
            rotational:     false,
            transport:      None,
            partitions:     Vec::new(),
            read_bytes_per_sec:   0.0,
            write_bytes_per_sec:  0.0,
            read_iops:            0.0,
            write_iops:           0.0,
            io_util_pct:          0.0,
            avg_read_latency_ms:  0.0,
            avg_write_latency_ms: 0.0,
            read_history:      RingBuffer::new(1800),
            write_history:     RingBuffer::new(1800),
            util_history:      RingBuffer::new(1800),
            read_lat_history:  RingBuffer::new(1800),
            write_lat_history: RingBuffer::new(1800),
            temp_history:      RingBuffer::new(1800),
            smart:           None,
            smart_prev:      None,
            smart_polled_at: None,
        }
    }

    pub fn smart_status(&self) -> SmartStatus {
        self.smart
            .as_ref()
            .map(|s| s.status.clone())
            .unwrap_or(SmartStatus::Unknown)
    }

    pub fn temperature(&self) -> Option<i32> {
        self.smart.as_ref().and_then(|s| s.temperature)
    }

    pub fn infer_type(&mut self) {
        let tran = self.transport.as_deref().unwrap_or("").to_lowercase();
        self.dev_type = if tran == "nvme" {
            DeviceType::NVMe
        } else if self.rotational {
            DeviceType::HDD
        } else if tran == "sata" || tran == "sas" {
            DeviceType::SSD
        } else if self.name.starts_with("md")
            || self.name.starts_with("dm-")
            || self.name.starts_with("loop")
            || self.name.starts_with("zram")
        {
            DeviceType::Virtual
        } else if !self.rotational {
            DeviceType::SSD
        } else {
            DeviceType::Unknown
        };
    }
}
