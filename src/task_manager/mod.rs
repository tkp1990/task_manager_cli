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
struct TaskManagerSessionState {
    task_filter: String,
    special_task_filter: String,
}

static TASK_MANAGER_SESSION_STATE: OnceLock<Mutex<TaskManagerSessionState>> = OnceLock::new();

fn task_manager_session_state() -> &'static Mutex<TaskManagerSessionState> {
    TASK_MANAGER_SESSION_STATE.get_or_init(|| Mutex::new(TaskManagerSessionState::default()))
}

pub fn run_task_manager(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let log = log_config::get_logger();
    info!(log, "Starting Task Manager, inside task manager...");

    let db_path = crate::db::resolve_db_path(
        "TASK_MANAGER_DB_DIR",
        ".task_manager",
        "TASK_MANAGER_DB_FILENAME",
        "task_manager.db",
    );
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
    if let Ok(state) = task_manager_session_state().lock() {
        app.task_filter = state.task_filter.clone();
        app.special_task_filter = state.special_task_filter.clone();
        app.ensure_selected_visible();
    }

    let result = ui::run(&mut app, terminal);
    if let Ok(mut state) = task_manager_session_state().lock() {
        state.task_filter = app.task_filter.clone();
        state.special_task_filter = app.special_task_filter.clone();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{task_manager_session_state, TaskManagerSessionState};

    #[test]
    fn task_manager_session_state_round_trips_filters() {
        let mut state = task_manager_session_state()
            .lock()
            .expect("task manager session state lock should succeed");
        *state = TaskManagerSessionState {
            task_filter: "status:done".to_string(),
            special_task_filter: "fav:true".to_string(),
        };

        assert_eq!(state.task_filter, "status:done");
        assert_eq!(state.special_task_filter, "fav:true");
    }
}
