pub mod app;
pub mod ui;
use crate::log_config;
use slog::{error, info};
use std::env;
use std::fs;
use std::io::Stdout;
use std::path::PathBuf;
use tui::backend::CrosstermBackend;
use tui::Terminal;

pub fn run_task_manager(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let log = log_config::get_logger();
    info!(log, "Starting Task Manager, inside task manager...");

    let db_dir_str = env::var("TASK_MANAGER_DB_DIR").map_err(|e| {
        error!(log, "Missing TASK_MANAGER_DB_DIR environment variable"; "error" => %e);
        e
    })?;
    info!(log, "DB_DIR: {}", db_dir_str);
    let db_filename = env::var("TASK_MANAGER_DB_FILENAME").map_err(|e| {
        error!(log, "Missing TASK_MANAGER_DB_FILENAME environment variable"; "error" => %e);
        e
    })?;
    info!(log, "DB_FILENAME: {}", db_filename);
    let db_dir = PathBuf::from(&db_dir_str);
    info!(log, "DB_DIR: {:?}", db_dir);
    fs::create_dir_all(&db_dir)?;
    let db_path = db_dir.join(db_filename);

    info!(log, "DB_PATH: {:?}", db_path);
    let db_path_str = db_path
        .to_str()
        .expect("Path contains invalid Unicode")
        .to_string();
    info!(log, "DB_PATH_STR: {}", db_path_str);

    let mut app = app::App::new(&db_path_str)?;
    ui::run(&mut app, terminal)
}
