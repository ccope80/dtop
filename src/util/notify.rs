use crate::alerts::{Alert, Severity};

/// Fire a desktop notification via `notify-send` for newly-triggered alerts.
/// Best-effort: silently ignored if notify-send is not installed or DISPLAY is unset.
pub fn notify_send(alerts: &[&Alert]) {
    if alerts.is_empty() { return; }

    // Pick the highest severity to set urgency
    let highest = alerts.iter()
        .max_by(|a, b| a.severity.cmp(&b.severity))
        .unwrap();

    let urgency = match highest.severity {
        Severity::Critical => "critical",
        Severity::Warning  => "normal",
        Severity::Info     => "low",
    };

    let title = format!(
        "dtop: {} new alert{}",
        alerts.len(),
        if alerts.len() == 1 { "" } else { "s" }
    );
    let body = format!(
        "[{}] {}{}",
        highest.severity.label(), highest.prefix(), highest.message
    );

    let _ = std::process::Command::new("notify-send")
        .args(["--urgency", urgency, "--app-name", "dtop", &title, &body])
        .spawn();
}
