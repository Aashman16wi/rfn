use std::time::SystemTime;

pub fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{}{}", value as u64, UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

pub fn format_age(modified: SystemTime) -> String {
    let seconds = match SystemTime::now().duration_since(modified) {
        Ok(duration) => duration.as_secs(),
        Err(_) => return "now".to_string(),
    };

    match seconds {
        0..=59 => "now".to_string(),
        60..=3599 => format!("{}m", seconds / 60),
        3600..=86_399 => format!("{}h", seconds / 3600),
        86_400..=2_591_999 => format!("{}d", seconds / 86_400),
        2_592_000..=31_535_999 => format!("{}mo", seconds / 2_592_000),
        _ => format!("{}y", seconds / 31_536_000),
    }
}

pub fn format_permissions(mode: u32) -> String {
    let mut out = String::with_capacity(9);
    for (mask, letter) in [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ] {
        out.push(if mode & mask != 0 { letter } else { '-' });
    }
    out
}
