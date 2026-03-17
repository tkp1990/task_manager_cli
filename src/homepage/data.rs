use diesel::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, SystemTimeError};
use std::{error::Error, io};

use crate::db::notes::models::Note;
use crate::db::schema::{note, task, topic};
use crate::db::task_manager::models::{Task, Topic};
use crate::leadership_tools::{load_dashboard_snapshot, ToolKind as LeadershipTool};

use super::types::{FileScanSummary, HomepageDashboard, NotesDashboard, TaskDashboard};

pub fn load_dashboard() -> Result<HomepageDashboard, Box<dyn Error>> {
    Ok(HomepageDashboard {
        tasks: load_task_dashboard()?,
        notes: load_notes_dashboard()?,
        one_on_ones: load_dashboard_snapshot(LeadershipTool::OneOnOne)?,
        delegations: load_dashboard_snapshot(LeadershipTool::Delegation)?,
        decisions: load_dashboard_snapshot(LeadershipTool::Decision)?,
        refreshed_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

fn load_task_dashboard() -> Result<TaskDashboard, Box<dyn Error>> {
    let db_path = crate::db::resolve_db_path(
        "TASK_MANAGER_DB_DIR",
        ".task_manager",
        "TASK_MANAGER_DB_FILENAME",
        "task_manager.db",
    );
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let database_url = format!("sqlite://{}", path_to_str(&db_path)?);
    let pool = crate::db::establish_connection_pool(&database_url)?;
    {
        let mut conn = pool.get()?;
        crate::db::run_migrations(&mut conn)?;
    }
    let mut conn = pool.get()?;

    let topics = topic::table
        .order_by(topic::id.asc())
        .load::<Topic>(&mut conn)?;
    let tasks = task::table
        .order_by(task::updated_at.desc())
        .load::<Task>(&mut conn)?;

    Ok(TaskDashboard {
        db_path,
        topic_count: topics.len(),
        task_count: tasks.len(),
        open_count: tasks.iter().filter(|task| !task.completed).count(),
        done_count: tasks.iter().filter(|task| task.completed).count(),
        favourite_count: tasks.iter().filter(|task| task.favourite).count(),
        recent_tasks: tasks
            .iter()
            .take(6)
            .map(|task| {
                format!(
                    "{} [{}] {}",
                    if task.completed { "done" } else { "open" },
                    task.updated_at,
                    compact_text(&task.name, 40)
                )
            })
            .collect(),
    })
}

fn load_notes_dashboard() -> Result<NotesDashboard, Box<dyn Error>> {
    let db_path =
        crate::db::resolve_db_path("NOTES_DB_DIR", ".notes", "NOTES_DB_FILENAME", "notes.db");
    let notes_root = std::env::var("NOTES_ROOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".notes/files"));
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&notes_root)?;

    let database_url = format!("sqlite://{}", path_to_str(&db_path)?);
    let pool = crate::db::establish_connection_pool(&database_url)?;
    {
        let mut conn = pool.get()?;
        crate::db::run_migrations(&mut conn)?;
    }
    let mut conn = pool.get()?;

    let notes = note::table
        .order_by(note::updated_at.desc())
        .load::<Note>(&mut conn)?;
    let file_scan = scan_notes_tree(&notes_root)?;

    Ok(NotesDashboard {
        db_path,
        notes_root,
        db_note_count: notes.len(),
        file_count: file_scan.file_count,
        directory_count: file_scan.directory_count,
        recent_notes: notes
            .iter()
            .take(6)
            .map(|note| format!("[{}] {}", note.updated_at, compact_text(&note.title, 42)))
            .collect(),
        recent_files: file_scan.recent_files,
    })
}

pub fn scan_notes_tree(root: &Path) -> Result<FileScanSummary, Box<dyn Error>> {
    if !root.exists() {
        return Ok(FileScanSummary {
            file_count: 0,
            directory_count: 0,
            recent_files: Vec::new(),
        });
    }

    let mut directory_count = 0usize;
    let mut file_entries = Vec::new();
    collect_note_files(root, root, &mut directory_count, &mut file_entries)?;
    file_entries.sort_by(|left, right| right.0.cmp(&left.0));

    Ok(FileScanSummary {
        file_count: file_entries.len(),
        directory_count,
        recent_files: file_entries
            .into_iter()
            .take(6)
            .map(|(_, path)| path)
            .collect(),
    })
}

fn collect_note_files(
    root: &Path,
    dir: &Path,
    directory_count: &mut usize,
    file_entries: &mut Vec<(SystemTime, String)>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            *directory_count += 1;
            collect_note_files(root, &path, directory_count, file_entries)?;
        } else {
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            file_entries.push((modified, compact_text(&relative, 48)));
        }
    }
    Ok(())
}

pub fn compact_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let char_count = trimmed.chars().count();
    if char_count <= max_chars {
        return trimmed.to_string();
    }

    trimmed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}

fn compact_path(path: &Path, max_chars: usize) -> String {
    compact_text(&path.display().to_string(), max_chars)
}

fn path_to_str(path: &Path) -> Result<&str, io::Error> {
    path.to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Path contains invalid Unicode"))
}

pub(crate) fn support_lines_for_tasks(dashboard: &HomepageDashboard) -> [Vec<String>; 2] {
    [
        vec![
            format!("DB: {}", compact_path(&dashboard.tasks.db_path, 22)),
            format!("Topics: {}", dashboard.tasks.topic_count),
            format!("Favourites: {}", dashboard.tasks.favourite_count),
        ],
        vec![
            format!(
                "Notes: {} files / {} dirs",
                dashboard.notes.file_count, dashboard.notes.directory_count
            ),
            format!("DB: {}", compact_path(&dashboard.notes.db_path, 22)),
            format!("Root: {}", compact_path(&dashboard.notes.notes_root, 20)),
        ],
    ]
}

pub(crate) fn support_lines_for_notes(dashboard: &HomepageDashboard) -> [Vec<String>; 2] {
    [
        vec![
            format!("Notes DB: {}", compact_path(&dashboard.notes.db_path, 22)),
            format!(
                "Notes root: {}",
                compact_path(&dashboard.notes.notes_root, 18)
            ),
            format!("DB notes: {}", dashboard.notes.db_note_count),
        ],
        vec![
            format!("Open tasks: {}", dashboard.tasks.open_count),
            format!("Completed: {}", dashboard.tasks.done_count),
            format!("Favourites: {}", dashboard.tasks.favourite_count),
        ],
    ]
}

pub(crate) fn support_lines_for_leadership(
    snapshot: &crate::leadership_tools::DashboardSnapshot,
    store_width: usize,
    context_lines: [String; 3],
) -> [Vec<String>; 2] {
    [
        vec![
            format!("{}: {}", snapshot.stat_a_label, snapshot.stat_a_value),
            format!("{}: {}", snapshot.stat_b_label, snapshot.stat_b_value),
            format!("Store: {}", compact_path(&snapshot.store_path, store_width)),
        ],
        context_lines.into(),
    ]
}

#[allow(dead_code)]
fn _ignore_system_time_error(_: SystemTimeError) {}
