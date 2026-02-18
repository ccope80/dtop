use crate::models::smart::{NvmeHealth, SmartAttribute, SmartData, SmartStatus};
use serde_json::Value;
use std::process::Command;

/// Run `smartctl --json -a /dev/<name>` and parse the result.
/// Returns None if smartctl is unavailable or the device doesn't support SMART.
pub fn poll_device(name: &str) -> Option<SmartData> {
    let device = format!("/dev/{}", name);

    let out = Command::new("smartctl")
        .args(["--json=c", "-a", &device])
        .output()
        .ok()?;

    // smartctl returns non-zero exit codes even on success when some bits are set,
    // so we parse regardless of exit code.
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;

    // Overall health
    let passed = v["smart_status"]["passed"].as_bool().unwrap_or(false);
    let status = if passed { SmartStatus::Passed } else { SmartStatus::Failed };

    // Temperature
    let temperature = v["temperature"]["current"].as_i64().map(|t| t as i32);

    // Power-on hours
    let power_on_hours = v["power_on_time"]["hours"].as_u64();

    // ATA attributes
    let attributes = parse_ata_attributes(&v);

    // NVMe health log
    let nvme = parse_nvme_health(&v);

    let mut data = SmartData { status, temperature, power_on_hours, attributes, nvme };
    data.derive_status();
    Some(data)
}

fn parse_ata_attributes(v: &Value) -> Vec<SmartAttribute> {
    let table = match v["ata_smart_attributes"]["table"].as_array() {
        Some(t) => t,
        None    => return Vec::new(),
    };

    table.iter().filter_map(|entry| {
        let id    = entry["id"].as_u64()? as u32;
        let name  = entry["name"].as_str().unwrap_or("Unknown").to_string();
        let value = entry["value"].as_u64().unwrap_or(0) as u16;
        let worst = entry["worst"].as_u64().unwrap_or(0) as u16;
        let thresh = entry["thresh"].as_u64().unwrap_or(0) as u16;
        let prefail = entry["flags"]["prefailure"].as_bool().unwrap_or(false);
        let raw_value = entry["raw"]["value"].as_u64().unwrap_or(0);
        let raw_str   = entry["raw"]["string"].as_str().unwrap_or("").to_string();
        let when_failed = entry["when_failed"].as_str().unwrap_or("").to_string();

        Some(SmartAttribute { id, name, value, worst, thresh, prefail, raw_value, raw_str, when_failed })
    }).collect()
}

fn parse_nvme_health(v: &Value) -> Option<NvmeHealth> {
    let log = &v["nvme_smart_health_information_log"];
    if log.is_null() || !log.is_object() { return None; }

    Some(NvmeHealth {
        critical_warning:          log["critical_warning"].as_u64().unwrap_or(0) as u8,
        temperature_celsius:       log["temperature"].as_i64().unwrap_or(0) as i32,
        available_spare_pct:       log["available_spare"].as_u64().unwrap_or(100) as u8,
        available_spare_threshold: log["available_spare_threshold"].as_u64().unwrap_or(10) as u8,
        percentage_used:           log["percentage_used"].as_u64().unwrap_or(0) as u8,
        data_units_read:           log["data_units_read"].as_u64().unwrap_or(0),
        data_units_written:        log["data_units_written"].as_u64().unwrap_or(0),
        power_on_hours:            log["power_on_hours"].as_u64().unwrap_or(0),
        unsafe_shutdowns:          log["unsafe_shutdowns"].as_u64().unwrap_or(0),
        media_errors:              log["media_errors"].as_u64().unwrap_or(0),
        error_log_entries:         log["num_err_log_entries"].as_u64().unwrap_or(0),
    })
}
