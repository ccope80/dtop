use std::fs;

#[derive(Debug, Clone, Default)]
pub struct PsiMetric {
    pub avg10:  f32,
    pub avg60:  f32,
    pub avg300: f32,
}

/// Pressure Stall Information for a single resource (io / cpu / mem).
/// "some" = at least one task stalled; "full" = all runnable tasks stalled.
#[derive(Debug, Clone, Default)]
pub struct PsiResource {
    pub some: PsiMetric,
    pub full: PsiMetric,
}

/// Combined PSI snapshot for io, cpu, and memory.
#[derive(Debug, Clone, Default)]
pub struct SystemPressure {
    pub io:  PsiResource,
    pub cpu: PsiResource,
    pub mem: PsiResource,
}

fn parse_psi_file(path: &str) -> Option<PsiResource> {
    let text = fs::read_to_string(path).ok()?;
    let mut res = PsiResource::default();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let kind = parts.next()?;
        let metric = match kind {
            "some" => &mut res.some,
            "full" => &mut res.full,
            _      => continue,
        };
        for token in parts {
            if let Some(v) = token.strip_prefix("avg10=")  { metric.avg10  = v.parse().unwrap_or(0.0); }
            if let Some(v) = token.strip_prefix("avg60=")  { metric.avg60  = v.parse().unwrap_or(0.0); }
            if let Some(v) = token.strip_prefix("avg300=") { metric.avg300 = v.parse().unwrap_or(0.0); }
        }
    }
    Some(res)
}

/// Read PSI for io, cpu, and memory from /proc/pressure/*.
/// Returns None only if /proc/pressure/io is unavailable (kernel < 4.20 or not mounted).
pub fn read_pressure() -> Option<SystemPressure> {
    let io = parse_psi_file("/proc/pressure/io")?;
    let cpu = parse_psi_file("/proc/pressure/cpu").unwrap_or_default();
    let mem = parse_psi_file("/proc/pressure/memory").unwrap_or_default();
    Some(SystemPressure { io, cpu, mem })
}
