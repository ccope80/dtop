use crate::models::filesystem::Filesystem;
use anyhow::Result;

/// Filesystems to skip — not useful for sysadmins.
const SKIP_FS: &[&str] = &[
    "proc", "sysfs", "devpts", "tmpfs", "devtmpfs", "cgroup", "cgroup2",
    "pstore", "efivarfs", "securityfs", "debugfs", "tracefs", "bpf",
    "hugetlbfs", "mqueue", "fusectl", "configfs", "binfmt_misc",
    "overlay", "nsfs", "rpc_pipefs", "autofs", "squashfs",
];

const SKIP_MOUNT_PREFIX: &[&str] = &[
    "/proc", "/sys", "/dev", "/run/user", "/snap",
];

pub fn read_filesystems() -> Result<Vec<Filesystem>> {
    let mounts = parse_mounts()?;
    let mut out = Vec::new();

    for (device, mount, fs_type) in &mounts {
        if SKIP_FS.contains(&fs_type.as_str()) { continue; }
        if SKIP_MOUNT_PREFIX.iter().any(|p| mount.starts_with(p)) { continue; }
        // Skip loop-mounted snaps
        if device.starts_with("/dev/loop") { continue; }

        if let Ok(fs) = statvfs_for(device, mount, fs_type) {
            out.push(fs);
        }
    }

    // Sort by mount point
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    Ok(out)
}

fn parse_mounts() -> Result<Vec<(String, String, String)>> {
    let content = std::fs::read_to_string("/proc/mounts")?;
    let mut v = Vec::new();
    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 { continue; }
        v.push((fields[0].to_string(), fields[1].to_string(), fields[2].to_string()));
    }
    Ok(v)
}

fn statvfs_for(device: &str, mount: &str, fs_type: &str) -> Result<Filesystem> {
    use nix::sys::statvfs::statvfs;
    let stat = statvfs(mount)?;

    let frsize = stat.fragment_size() as u64;
    let total_bytes  = stat.blocks()            * frsize;
    let avail_bytes  = stat.blocks_available()  * frsize;
    let free_bytes   = stat.blocks_free()        * frsize;
    let used_bytes   = total_bytes.saturating_sub(free_bytes);

    Ok(Filesystem {
        device:       device.to_string(),
        mount:        mount.to_string(),
        fs_type:      fs_type.to_string(),
        total_bytes,
        used_bytes,
        avail_bytes,
        total_inodes: stat.files(),
        free_inodes:  stat.files_free(),
        fill_rate_bps:   None,
        days_until_full: None,
    })
}
