mod draw;
mod events;

use std::error::Error;
use tui::{backend::Backend, Terminal};

use crate::task_manager::app::App;

use events::{handle_key, UiAction};

pub fn run<B: Backend>(app: &mut App, terminal: &mut Terminal<B>) -> Result<(), Box<dyn Error>> {
    crate::common::tui::run_event_loop(
        terminal,
        app,
        |f, app| draw::draw_ui(f, app),
        handle_key,
        |action, _, _| {
            Ok(match action {
                UiAction::Continue => false,
                UiAction::Exit => true,
            })
        },
    )
}
