mod draw;
mod events;

use tui::{backend::Backend, Terminal};

use crate::notes::app::App;

use events::{handle_key, UiAction};

pub fn run<B: Backend>(
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::common::tui::run_event_loop_with_tick(
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
        |app| app.maybe_autosave(),
    )
}
