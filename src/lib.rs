mod common;
pub mod db;
mod filter_presets;
mod homepage;
pub mod leadership_tools;
mod log_config;
pub mod notes;
pub mod task_manager;
mod ui_style;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use slog::info;
use std::io;
use tui::{backend::CrosstermBackend, Terminal};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let log = log_config::init_logger();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    info!(log, "Starting Task Manager...");
    let res = homepage::run_homepage(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    res
}
