use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use super::helpers::{blank_dash, next_id, summarize_inline, timestamp, value_at};
use super::{DashboardSnapshot, ToolKind, ToolRecord};

pub fn load_dashboard_snapshot(kind: ToolKind) -> Result<DashboardSnapshot, Box<dyn Error>> {
    let spec = kind.spec();
    let store_path = store_path_for(kind);
    let records = load_records(&store_path)?;

    let (stat_a_value, stat_b_value, stat_a_label, stat_b_label) = match kind {
        ToolKind::OneOnOne => (
            records
                .iter()
                .filter(|record| !value_at(record, 5).trim().is_empty())
                .count(),
            records
                .iter()
                .filter(|record| {
                    let values = spec.normalize_values(&record.values);
                    !values
                        .get(10)
                        .map(|value| value.trim().is_empty())
                        .unwrap_or(true)
                })
                .count(),
            "scheduled",
            "action items",
        ),
        ToolKind::Delegation => (
            records
                .iter()
                .filter(|record| value_at(record, 2) != "Done")
                .count(),
            records
                .iter()
                .filter(|record| {
                    let values = spec.normalize_values(&record.values);
                    spec.needs_follow_up(&values)
                })
                .count(),
            "open",
            "follow-up due",
        ),
        ToolKind::Decision => (
            records
                .iter()
                .filter(|record| value_at(record, 2) == "Decided")
                .count(),
            records
                .iter()
                .filter(|record| {
                    let values = spec.normalize_values(&record.values);
                    spec.review_due(&values)
                })
                .count(),
            "decided",
            "review due",
        ),
    };

    Ok(DashboardSnapshot {
        store_path,
        count: records.len(),
        recent_items: records
            .iter()
            .take(5)
            .map(|record| spec.list_summary(&spec.normalize_values(&record.values)))
            .zip(
                records
                    .iter()
                    .take(5)
                    .map(|record| value_at(record, 0).to_string()),
            )
            .map(|(summary, title)| format!("{title} | {summary}"))
            .collect(),
        stat_a_label,
        stat_a_value,
        stat_b_label,
        stat_b_value,
    })
}

pub fn store_path_for(kind: ToolKind) -> PathBuf {
    let base = std::env::var("LEADERSHIP_TOOLS_DIR").unwrap_or_else(|_| ".leadership".to_string());
    PathBuf::from(base).join(kind.spec().store_filename)
}

pub fn load_records(path: &Path) -> Result<Vec<ToolRecord>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&content)?)
}

pub fn load_task_summary(task_id: i32) -> Result<Option<(String, String)>, Box<dyn Error>> {
    let db_path = crate::db::resolve_db_path(
        "TASK_MANAGER_DB_DIR",
        ".task_manager",
        "TASK_MANAGER_DB_FILENAME",
        "task_manager.db",
    );
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = crate::db::establish_connection_pool(&db_url)?;
    let ops = crate::db::task_manager::operations::DbOperations::new(pool);

    Ok(ops.find_task(task_id)?.map(|task| {
        let status = if task.completed { "done" } else { "open" };
        (
            task.name,
            format!("#{task_id} | {status} | {}", blank_dash(&task.description)),
        )
    }))
}

pub fn load_note_summary(note_id: i32) -> Result<Option<(String, String)>, Box<dyn Error>> {
    let db_path =
        crate::db::resolve_db_path("NOTES_DB_DIR", ".notes", "NOTES_DB_FILENAME", "notes.db");
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = crate::db::establish_connection_pool(&db_url)?;
    let ops = crate::db::notes::operations::DbOperations::new(pool);

    Ok(ops.find_note(note_id)?.map(|note| {
        (
            note.title,
            format!("#{note_id} | {}", summarize_inline(&note.content, 48)),
        )
    }))
}

pub fn append_delegations_from_sync(
    person: &str,
    team: &str,
    purpose: &str,
    meeting_type: &str,
    actions: &[String],
) -> Result<(), Box<dyn Error>> {
    let store_path = store_path_for(ToolKind::Delegation);
    if let Some(parent) = store_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut records = load_records(&store_path)?;
    let context = format!(
        "{} | {} | {}",
        blank_dash(team),
        blank_dash(purpose),
        blank_dash(meeting_type)
    );

    for action in actions {
        records.insert(
            0,
            ToolRecord {
                id: next_id(&records),
                values: vec![
                    action.clone(),
                    person.to_string(),
                    "Delegated".to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                    context.clone(),
                ],
                updated_at: timestamp(),
            },
        );
    }

    let content = serde_json::to_string_pretty(&records)?;
    fs::write(store_path, content)?;
    Ok(())
}
