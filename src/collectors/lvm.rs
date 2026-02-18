use crate::models::volume::{LvmState, LvmVg, LvmLv, LvmPv};
use std::process::Command;

/// Try to collect LVM state. Returns None if LVM is not installed or has no VGs.
pub fn read_lvm() -> Option<LvmState> {
    let vgs = read_vgs()?;
    if vgs.is_empty() { return None; }
    let lvs = read_lvs().unwrap_or_default();
    let pvs = read_pvs().unwrap_or_default();
    Some(LvmState { vgs, lvs, pvs })
}

fn read_vgs() -> Option<Vec<LvmVg>> {
    let out = Command::new("vgs")
        .args(["--noheadings", "--nosuffix", "--units", "b",
               "-o", "vg_name,vg_size,vg_free,pv_count,lv_count"])
        .output()
        .ok()?;

    if !out.status.success() { return None; }

    let text = String::from_utf8_lossy(&out.stdout);
    let vgs: Vec<LvmVg> = text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 5 { return None; }
            Some(LvmVg {
                name:       f[0].to_string(),
                size_bytes: f[1].parse().unwrap_or(0),
                free_bytes: f[2].parse().unwrap_or(0),
                pv_count:   f[3].parse().unwrap_or(0),
                lv_count:   f[4].parse().unwrap_or(0),
            })
        })
        .collect();

    if vgs.is_empty() { None } else { Some(vgs) }
}

fn read_lvs() -> Option<Vec<LvmLv>> {
    let out = Command::new("lvs")
        .args(["--noheadings", "--nosuffix", "--units", "b",
               "-o", "lv_name,vg_name,lv_size,lv_attr,lv_path"])
        .output()
        .ok()?;

    if !out.status.success() { return None; }

    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 5 { return None; }
            Some(LvmLv {
                name:       f[0].to_string(),
                vg_name:    f[1].to_string(),
                size_bytes: f[2].parse().unwrap_or(0),
                attr:       f[3].to_string(),
                path:       f[4].to_string(),
            })
        })
        .collect())
}

fn read_pvs() -> Option<Vec<LvmPv>> {
    let out = Command::new("pvs")
        .args(["--noheadings", "--nosuffix", "--units", "b",
               "-o", "pv_name,vg_name,pv_size,pv_free"])
        .output()
        .ok()?;

    if !out.status.success() { return None; }

    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 4 { return None; }
            Some(LvmPv {
                name:       f[0].to_string(),
                vg_name:    f[1].to_string(),
                size_bytes: f[2].parse().unwrap_or(0),
                free_bytes: f[3].parse().unwrap_or(0),
            })
        })
        .collect())
}
