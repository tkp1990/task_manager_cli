pub mod app;
pub mod ui;
use crate::log_config;
use slog::info;
use std::fs;
use std::io;
use std::io::Stdout;
use std::sync::{Mutex, OnceLock};
use tui::backend::CrosstermBackend;
use tui::Terminal;

#[derive(Clone, Default)]
struct NotesSessionState {
    note_filter: String,
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
    if let Some(db_dir) = db_path.parent() {
        info!(log, "DB_DIR: {:?}", db_dir);
        fs::create_dir_all(db_dir)?;
    }

    info!(log, "DB_PATH: {:?}", db_path);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Path contains invalid Unicode"))?
        .to_string();
    info!(log, "DB_PATH_STR: {}", db_path_str);

    let mut app = app::App::new(&db_path_str)?;
    if let Ok(state) = notes_session_state().lock() {
        app.note_filter = state.note_filter.clone();
        app.ensure_selected_visible();
    }

    let result = ui::run(&mut app, terminal);
    if let Ok(mut state) = notes_session_state().lock() {
        state.note_filter = app.note_filter.clone();
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
        };

        assert_eq!(state.note_filter, "title:alpha");
    }
}
