pub mod app;
pub mod ui;

use std::io::Stdout;
use tui::backend::CrosstermBackend;
use tui::Terminal;

pub use app::{load_dashboard_snapshot, DashboardSnapshot, ToolKind};

pub fn run_tool(
    kind: ToolKind,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = app::App::new(kind)?;
    ui::run(&mut app, terminal)
}
