/// NFS/network mount statistics parsed from /proc/self/mountstats.
#[derive(Debug, Clone)]
pub struct NfsMountStats {
    pub device:            String,   // "server:/export"
    pub mount:             String,   // "/mnt/nfs"
    pub fstype:            String,   // "nfs4"
    pub age_secs:          u64,
    pub read_ops:          u64,
    pub write_ops:         u64,
    pub read_rtt_ms:       f64,      // average RTT per read op (ms)
    pub write_rtt_ms:      f64,      // average RTT per write op (ms)
    pub server_bytes_read: u64,
    pub server_bytes_written: u64,
}

impl NfsMountStats {
    #[allow(dead_code)]
    pub fn read_latency_label(&self) -> String {
        if self.read_rtt_ms < 1.0 { format!("{:.2}ms", self.read_rtt_ms) }
        else                       { format!("{:.1}ms", self.read_rtt_ms) }
    }

    #[allow(dead_code)]
    pub fn write_latency_label(&self) -> String {
        if self.write_rtt_ms < 1.0 { format!("{:.2}ms", self.write_rtt_ms) }
        else                        { format!("{:.1}ms", self.write_rtt_ms) }
    }

    pub fn status_str(&self) -> &'static str {
        let rtt = self.read_rtt_ms.max(self.write_rtt_ms);
        if rtt == 0.0    { "—" }
        else if rtt < 5.0   { "OK" }
        else if rtt < 50.0  { "SLOW" }
        else                { "DEGRADED" }
    }
}

/// Parse /proc/self/mountstats and return only NFS/NFS4 mounts.
pub fn read_nfs_mounts() -> Vec<NfsMountStats> {
    let text = match std::fs::read_to_string("/proc/self/mountstats") {
        Ok(t)  => t,
        Err(_) => return Vec::new(),
    };

    let mut mounts = Vec::new();
    let mut current: Option<NfsMountStats> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        // "device server:/path mounted on /mnt/nfs with fstype nfs4 statvers=1.1"
        if trimmed.starts_with("device ") {
            // flush previous mount
            if let Some(m) = current.take() {
                if m.fstype.starts_with("nfs") {
                    mounts.push(m);
                }
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            // device <dev> mounted on <mnt> with fstype <fs> ...
            if parts.len() >= 8 {
                let device = parts[1].to_string();
                let mount  = parts[4].to_string();
                let fstype = parts[7].to_string();
                if fstype.starts_with("nfs") {
                    current = Some(NfsMountStats {
                        device,
                        mount,
                        fstype,
                        age_secs: 0,
                        read_ops: 0,
                        write_ops: 0,
                        read_rtt_ms: 0.0,
                        write_rtt_ms: 0.0,
                        server_bytes_read: 0,
                        server_bytes_written: 0,
                    });
                }
            }
            continue;
        }

        if current.is_none() { continue; }

        // "age: 12345"
        if trimmed.starts_with("age:") {
            if let Some(m) = &mut current {
                let rest = trimmed.trim_start_matches("age:").trim();
                m.age_secs = rest.parse().unwrap_or(0);
            }
            continue;
        }

        // "bytes: normread normwrite directread directwrite serverread serverwrite readpages writepages"
        if trimmed.starts_with("bytes:") {
            if let Some(m) = &mut current {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                // bytes: normread normwrite directread directwrite serverread serverwrite ...
                if parts.len() >= 7 {
                    m.server_bytes_read    = parts[5].parse().unwrap_or(0);
                    m.server_bytes_written = parts[6].parse().unwrap_or(0);
                }
            }
            continue;
        }

        // per-op stats lines, e.g.:
        // "READ: ops ntrans timeouts bytes_sent bytes_recv queue_ms rtt_ms execute_ms"
        // Fields: [0]=opname: [1]=ops [2]=ntrans [3]=timeouts [4]=bytes_sent [5]=bytes_recv
        //         [6]=queue_ms [7]=rtt_ms [8]=execute_ms
        // Note: rtt_ms and others are in milliseconds * 1000 (actually they are in
        //       milliseconds already in newer kernel versions; field format varies)
        // Safer: treat rtt_ms as the cumulative ms, divide by ops to get avg
        let upper = trimmed.to_uppercase();
        if upper.starts_with("READ:") || upper.starts_with("WRITE:") {
            if let Some(m) = &mut current {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                // Need at least: opname ops ntrans timeouts bytes_sent bytes_recv queue_ms rtt_ms
                if parts.len() >= 8 {
                    let ops: u64      = parts[1].parse().unwrap_or(0);
                    let rtt_total: f64 = parts[7].parse().unwrap_or(0.0);
                    let avg_rtt = if ops > 0 { rtt_total / ops as f64 } else { 0.0 };

                    if upper.starts_with("READ:") {
                        m.read_ops   = ops;
                        m.read_rtt_ms = avg_rtt;
                    } else {
                        m.write_ops   = ops;
                        m.write_rtt_ms = avg_rtt;
                    }
                }
            }
            continue;
        }
    }

    // flush last
    if let Some(m) = current {
        if m.fstype.starts_with("nfs") {
            mounts.push(m);
        }
    }

    mounts
}
