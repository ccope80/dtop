use crate::models::device::BlockDevice;
use crate::models::smart::SmartStatus;
use ratatui::style::Style;
use crate::ui::theme::Theme;

/// Compute a 0-100 health score from SMART data.
/// 100 = pristine / unscored (no SMART data), 0 = hard failure.
/// Points are deducted for bad attributes, high temperature, and NVMe wear.
pub fn health_score(dev: &BlockDevice) -> u8 {
    let smart = match &dev.smart {
        Some(s) => s,
        None    => return 100,  // unscored — no penalty
    };

    if smart.status == SmartStatus::Failed {
        return 0;
    }

    let mut score: i32 = 100;

    // Base deduction for Warning status
    if smart.status == SmartStatus::Warning {
        score -= 10;
    }

    // Temperature penalty
    if let Some(t) = smart.temperature {
        if dev.rotational {
            if t >= 60      { score -= 20; }
            else if t >= 50 { score -= 10; }
        } else {
            if t >= 70      { score -= 20; }
            else if t >= 55 { score -= 10; }
        }
    }

    // ATA SMART critical attribute penalties
    for attr in &smart.attributes {
        match attr.id {
            5   => { // Reallocated_Sector_Ct
                if attr.raw_value > 100 { score -= 30; }
                else if attr.raw_value > 0 { score -= 15; }
            }
            197 => { // Current_Pending_Sector
                if attr.raw_value > 0 { score -= 25; }
            }
            198 => { // Offline_Uncorrectable
                if attr.raw_value > 0 { score -= 40; }
            }
            9   => {} // Power-on hours — handled via lifespan estimate
            _ => {}
        }
    }

    // NVMe-specific penalties
    if let Some(nvme) = &smart.nvme {
        match nvme.percentage_used {
            90..=u8::MAX => { score -= 30; }
            70..=89      => { score -= 15; }
            50..=69      => { score -=  5; }
            _            => {}
        }
        if nvme.media_errors > 0 { score -= 25; }
        if nvme.available_spare_pct < nvme.available_spare_threshold { score -= 20; }
    }

    score.clamp(0, 100) as u8
}

/// Return whether this device has real SMART data (score is meaningful).
pub fn has_smart(dev: &BlockDevice) -> bool {
    dev.smart.is_some()
}

/// Color style for a health score value.
pub fn score_style(score: u8, theme: &Theme) -> Style {
    if score >= 80 { theme.ok }
    else if score >= 50 { theme.warn }
    else { theme.crit }
}

/// Short display string for the score.
pub fn score_str(dev: &BlockDevice) -> String {
    if dev.smart.is_none() {
        return "  ?".to_string();
    }
    format!("{:>3}", health_score(dev))
}
