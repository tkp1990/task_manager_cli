mod draw;
mod events;

use crossterm::event::{self, Event};
use std::error::Error;
use std::io::Stdout;
use std::time::{Duration, Instant};
use tui::backend::CrosstermBackend;
use tui::Terminal;

use crate::leadership_tools::app::App;

use events::{handle_key, UiAction};

pub fn run(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    terminal.clear()?;

    loop {
        terminal.draw(|f| draw::draw_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match handle_key(app, key)? {
                    UiAction::Continue => {}
                    UiAction::Exit => break,
                    UiAction::OpenTask(task_id) => {
                        crate::task_manager::run_task_manager_with_focus(terminal, task_id)?;
                        terminal.clear()?;
                    }
                    UiAction::OpenNote(note_id) => {
                        crate::notes::run_notes_app_with_focus(terminal, note_id)?;
                        terminal.clear()?;
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}
