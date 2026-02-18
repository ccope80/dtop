use crate::alerts::{Alert, Severity};

/// Fire an HTTP POST to `url` with a Slack/Discord-compatible JSON payload.
/// Runs in a detached background thread so it never blocks the UI.
pub fn notify(alerts: &[Alert], url: &str, notify_warning: bool) {
    if url.is_empty() { return; }

    let relevant: Vec<&Alert> = alerts.iter().filter(|a| {
        a.severity == Severity::Critical || (notify_warning && a.severity == Severity::Warning)
    }).collect();

    if relevant.is_empty() { return; }

    let text = relevant.iter()
        .map(|a| format!("[{}] {}{}", a.severity.label(), a.prefix(), a.message))
        .collect::<Vec<_>>()
        .join("\\n");

    // Slack/Discord both accept {"text": "..."} as a minimal payload.
    let payload = format!("{{\"text\":\"{}\"}}", text.replace('"', "\\\""));
    let url = url.to_string();

    std::thread::spawn(move || {
        let _ = std::process::Command::new("curl")
            .args([
                "-s", "--max-time", "10",
                "-X", "POST",
                "-H", "Content-Type: application/json",
                "-d", &payload,
                &url,
            ])
            .output();
    });
}
