use chrono::{Duration, Local, NaiveDate};

use super::ToolRecord;

pub fn parse_ymd(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()
}

pub fn parse_linked_ids(value: &str) -> Vec<i32> {
    let mut ids = Vec::new();

    for token in value
        .split(|c: char| c == ',' || c == '|' || c.is_whitespace())
        .filter(|part| !part.trim().is_empty())
    {
        let token = token
            .trim()
            .trim_start_matches("note:")
            .trim_start_matches("task:")
            .trim_start_matches('#');
        if let Ok(id) = token.parse::<i32>() {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }

    ids
}

pub fn parse_action_items(value: &str) -> Vec<String> {
    value
        .split('|')
        .flat_map(|segment| segment.split('\n'))
        .map(|item| item.trim().trim_start_matches('-').trim())
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect()
}

pub fn cadence_duration(value: &str) -> Duration {
    let normalized = value.trim().to_lowercase();
    if normalized.contains("biweekly") || normalized.contains("bi-weekly") {
        Duration::days(14)
    } else if normalized.contains("monthly") {
        Duration::days(28)
    } else {
        Duration::days(7)
    }
}

pub fn merge_rollover_items(existing: &str, agenda: &str) -> String {
    let agenda = agenda.trim();
    let existing = existing.trim();

    match (existing.is_empty(), agenda.is_empty()) {
        (_, true) => existing.to_string(),
        (true, false) => agenda.to_string(),
        (false, false) => format!("{existing} | {agenda}"),
    }
}

pub fn contains_value(value: Option<&String>, needle: &str) -> bool {
    value
        .map(|value| value.to_lowercase().contains(needle))
        .unwrap_or(false)
}

pub fn contains_str(value: Option<&str>, needle: &str) -> bool {
    value
        .map(|value| value.to_lowercase().contains(needle))
        .unwrap_or(false)
}

pub fn next_id(records: &[ToolRecord]) -> u64 {
    records.iter().map(|record| record.id).max().unwrap_or(0) + 1
}

pub fn timestamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn blank_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "-"
    } else {
        value
    }
}

pub fn with_prefix(prefix: &str, value: &str) -> String {
    if value.trim().is_empty() {
        "-".to_string()
    } else {
        format!("{prefix}{value}")
    }
}

pub fn value_at(record: &ToolRecord, index: usize) -> &str {
    record
        .values
        .get(index)
        .map(|value| value.as_str())
        .unwrap_or("")
}

pub fn summarize_inline(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = compact.chars();
    let summary = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{summary}...")
    } else if summary.is_empty() {
        "-".to_string()
    } else {
        summary
    }
}
