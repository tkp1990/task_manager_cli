pub mod app;
pub mod ui;
use std::io::Stdout;
use tui::backend::CrosstermBackend;
use tui::Terminal;

pub fn run_task_manager(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = app::App::new("task_manager.db")?;
    ui::run(&mut app, terminal)
}
