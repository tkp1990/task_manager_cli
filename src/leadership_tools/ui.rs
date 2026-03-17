mod draw;
mod events;

use std::error::Error;
use std::io::Stdout;
use tui::{backend::CrosstermBackend, Terminal};

use crate::leadership_tools::app::App;

use events::{handle_key, UiAction};

pub fn run(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    crate::common::tui::run_event_loop(
        terminal,
        app,
        |f, app| draw::draw_ui(f, app),
        handle_key,
        |action, _, terminal| match action {
            UiAction::Continue => Ok(false),
            UiAction::Exit => Ok(true),
            UiAction::OpenTask(task_id) => {
                crate::task_manager::run_task_manager_with_focus(terminal, task_id)?;
                terminal.clear()?;
                Ok(false)
            }
            UiAction::OpenNote(note_id) => {
                crate::notes::run_notes_app_with_focus(terminal, note_id)?;
                terminal.clear()?;
                Ok(false)
            }
        },
    )
}
