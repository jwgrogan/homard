use tracing::warn;

/// Parse HEARTBEAT.md into schedule-like entries.
/// Recognizes patterns like:
/// "# Every Morning (9am)" -> cron "0 9 * * *"
/// "# Every Friday (5pm)" -> cron "0 17 * * 5"
/// "# Cron: 0 9 * * *" -> literal cron expression
pub fn parse_heartbeat(content: &str) -> Vec<(String, String, Vec<String>)> {
    // Returns: Vec<(name, cron_expression, checklist_items)>
    let mut entries = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_cron: Option<String> = None;
    let mut current_items: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("# ") || trimmed.starts_with("## ") {
            // Save previous entry
            if let (Some(name), Some(cron)) = (current_name.take(), current_cron.take()) {
                if !current_items.is_empty() {
                    entries.push((name, cron, current_items.clone()));
                }
            }
            current_items.clear();

            let header = trimmed.trim_start_matches('#').trim();

            // Try explicit cron syntax first: "Cron: 0 9 * * *"
            if let Some(cron) = header.strip_prefix("Cron:").or_else(|| header.strip_prefix("cron:")) {
                current_name = Some(header.to_string());
                current_cron = Some(cron.trim().to_string());
                continue;
            }

            // Try natural language patterns
            let lower = header.to_lowercase();
            let cron = parse_natural_schedule(&lower);

            if let Some(c) = cron {
                current_name = Some(header.to_string());
                current_cron = Some(c);
            } else {
                warn!("HEARTBEAT.md: unrecognized schedule pattern: '{}'", header);
                current_name = None;
                current_cron = None;
            }
        } else if trimmed.starts_with("- ") {
            if current_cron.is_some() {
                current_items.push(trimmed[2..].trim().to_string());
            }
        }
    }

    // Save last entry
    if let (Some(name), Some(cron)) = (current_name, current_cron) {
        if !current_items.is_empty() {
            entries.push((name, cron, current_items));
        }
    }

    entries
}

fn parse_natural_schedule(lower: &str) -> Option<String> {
    // "every morning (9am)" -> "0 9 * * *"
    // "every evening (6pm)" -> "0 18 * * *"
    // "every hour" -> "0 * * * *"
    // "daily (9am)" -> "0 9 * * *"
    // "every monday (9am)" -> "0 9 * * 1"
    // "every friday (5pm)" -> "0 17 * * 5"

    let hour = extract_hour(lower);

    if lower.contains("every morning") {
        return Some(format!("0 {} * * *", hour.unwrap_or(9)));
    }
    if lower.contains("every evening") {
        return Some(format!("0 {} * * *", hour.unwrap_or(18)));
    }
    if lower.contains("every hour") {
        return Some("0 * * * *".to_string());
    }
    if lower.contains("daily") {
        return Some(format!("0 {} * * *", hour.unwrap_or(9)));
    }

    // Day-specific
    let days = [
        ("monday", 1), ("tuesday", 2), ("wednesday", 3),
        ("thursday", 4), ("friday", 5), ("saturday", 6), ("sunday", 0),
    ];
    for (day_name, dow) in &days {
        if lower.contains(day_name) {
            return Some(format!("0 {} * * {}", hour.unwrap_or(9), dow));
        }
    }

    None
}

fn extract_hour(text: &str) -> Option<u32> {
    // Match patterns like "(9am)", "(5pm)", "(14:00)", "(9:30am)"
    use regex_lite::Regex;

    // Try "Xam" or "Xpm"
    if let Ok(re) = Regex::new(r"(\d{1,2})\s*(am|pm)") {
        if let Some(caps) = re.captures(text) {
            let h: u32 = caps[1].parse().ok()?;
            let ampm = &caps[2];
            let hour = match ampm {
                "am" => if h == 12 { 0 } else { h },
                "pm" => if h == 12 { 12 } else { h + 12 },
                _ => h,
            };
            return Some(hour);
        }
    }

    // Try 24h format "HH:MM"
    if let Ok(re) = Regex::new(r"(\d{1,2}):(\d{2})") {
        if let Some(caps) = re.captures(text) {
            return caps[1].parse().ok();
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heartbeat() {
        let content = r#"# Every Morning (9am)
- Check calendar
- Check notifications

# Every Friday (5pm)
- Weekly summary

# Cron: 0 12 * * *
- Midday check
"#;
        let entries = parse_heartbeat(content);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].1, "0 9 * * *");
        assert_eq!(entries[0].2.len(), 2);
        assert_eq!(entries[1].1, "0 17 * * 5");
        assert_eq!(entries[2].1, "0 12 * * *");
    }

    #[test]
    fn test_extract_hour() {
        assert_eq!(extract_hour("every morning (9am)"), Some(9));
        assert_eq!(extract_hour("every friday (5pm)"), Some(17));
        assert_eq!(extract_hour("daily (14:00)"), Some(14));
        assert_eq!(extract_hour("every hour"), None);
    }
}
