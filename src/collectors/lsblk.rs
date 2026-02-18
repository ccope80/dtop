use crate::models::device::Partition;
use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

/// Run `lsblk --json --bytes` and return top-level disk devices with their partitions.
pub fn run_lsblk() -> Result<Vec<LsblkDisk>> {
    let out = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "-o",
            "NAME,TYPE,SIZE,FSTYPE,MOUNTPOINT,MODEL,SERIAL,ROTA,TRAN,VENDOR",
        ])
        .output()
        .context("lsblk not found")?;

    let v: Value = serde_json::from_slice(&out.stdout)?;
    let devices = v["blockdevices"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut disks = Vec::new();
    for dev in &devices {
        let name     = dev["name"].as_str().unwrap_or("").to_string();
        let dev_type = dev["type"].as_str().unwrap_or("");
        if name.is_empty() { continue; }
        if dev_type != "disk" { continue; }

        let size    = dev["size"].as_u64().unwrap_or(0);
        let model   = str_opt(&dev["model"]);
        let serial  = str_opt(&dev["serial"]);
        let rota    = dev["rota"].as_bool().unwrap_or(false);
        let tran    = str_opt(&dev["tran"]);

        let partitions = parse_children(dev);

        disks.push(LsblkDisk { name, size, model, serial, rotational: rota, transport: tran, partitions });
    }
    Ok(disks)
}

fn parse_children(dev: &Value) -> Vec<Partition> {
    let children = match dev["children"].as_array() {
        Some(c) => c,
        None    => return Vec::new(),
    };

    children.iter().filter_map(|child| {
        let name = child["name"].as_str()?.to_string();
        let size = child["size"].as_u64().unwrap_or(0);
        let fs_type    = str_opt(&child["fstype"]);
        let mountpoint = str_opt(&child["mountpoint"]);
        Some(Partition { name, size, fs_type, mountpoint })
    }).collect()
}

/// Metadata for one top-level disk device from lsblk.
pub struct LsblkDisk {
    pub name:       String,
    pub size:       u64,
    pub model:      Option<String>,
    pub serial:     Option<String>,
    pub rotational: bool,
    pub transport:  Option<String>,
    pub partitions: Vec<Partition>,
}

fn str_opt(v: &Value) -> Option<String> {
    v.as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
