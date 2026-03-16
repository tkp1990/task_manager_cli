pub mod app;
pub mod ui;
use crate::log_config;
use crate::notes::app::NotesView;
use slog::info;
use std::fs;
use std::io;
use std::io::Stdout;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tui::backend::CrosstermBackend;
use tui::Terminal;

#[derive(Clone, Default)]
struct NotesSessionState {
    note_filter: String,
    active_view: Option<NotesView>,
    current_dir: Option<PathBuf>,
    file_search_query: String,
    selected_file_path: Option<PathBuf>,
}

static NOTES_SESSION_STATE: OnceLock<Mutex<NotesSessionState>> = OnceLock::new();

fn notes_session_state() -> &'static Mutex<NotesSessionState> {
    NOTES_SESSION_STATE.get_or_init(|| Mutex::new(NotesSessionState::default()))
}

pub fn run_notes_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let log = log_config::get_logger();
    info!(log, "Starting Notes App...");

    let db_path =
        crate::db::resolve_db_path("NOTES_DB_DIR", ".notes", "NOTES_DB_FILENAME", "notes.db");
    let notes_root = std::env::var("NOTES_ROOT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(".notes/files"));
    if let Some(db_dir) = db_path.parent() {
        info!(log, "DB_DIR: {:?}", db_dir);
        fs::create_dir_all(db_dir)?;
    }
    fs::create_dir_all(&notes_root)?;

    info!(log, "DB_PATH: {:?}", db_path);
    info!(log, "NOTES_ROOT: {:?}", notes_root);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Path contains invalid Unicode"))?
        .to_string();
    info!(log, "DB_PATH_STR: {}", db_path_str);

    let mut app = app::App::new_with_notes_root(&db_path_str, notes_root)?;
    if let Ok(state) = notes_session_state().lock() {
        app.note_filter = state.note_filter.clone();
        app.ensure_selected_visible();
        if let Some(active_view) = state.active_view {
            app.active_view = active_view;
        }
        if let Some(current_dir) = &state.current_dir {
            if current_dir.starts_with(&app.notes_root) && current_dir.exists() {
                app.current_dir = current_dir.clone();
            }
        }
        app.load_file_entries()?;
        if !state.file_search_query.is_empty() {
            app.set_file_search_query(&state.file_search_query)?;
        }
        if let Some(path) = &state.selected_file_path {
            if path.starts_with(&app.notes_root) && path.exists() {
                app.select_file_entry_path(path);
            }
        }
    }

    let result = ui::run(&mut app, terminal);
    if let Ok(mut state) = notes_session_state().lock() {
        state.note_filter = app.note_filter.clone();
        state.active_view = Some(app.active_view);
        state.current_dir = Some(app.current_dir.clone());
        state.file_search_query = app.file_search_query.clone();
        state.selected_file_path = app.selected_file_entry().map(|entry| entry.path.clone());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{notes_session_state, NotesSessionState};

    #[test]
    fn notes_session_state_round_trips_filter() {
        let mut state = notes_session_state()
            .lock()
            .expect("notes session state lock should succeed");
        *state = NotesSessionState {
            note_filter: "title:alpha".to_string(),
            active_view: Some(super::app::NotesView::Files),
            current_dir: Some(std::path::PathBuf::from(".notes/files")),
            file_search_query: "roadmap".to_string(),
            selected_file_path: Some(std::path::PathBuf::from(".notes/files/roadmap.md")),
        };

        assert_eq!(state.note_filter, "title:alpha");
        assert_eq!(state.file_search_query, "roadmap");
    }
}
