mod draw;
mod events;

use crossterm::event::{self, Event};
use std::time::{Duration, Instant};
use tui::{backend::Backend, Terminal};

use crate::notes::app::App;

use events::{handle_key, UiAction};

pub fn run<B: Backend>(
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    terminal.clear()?;
    loop {
        terminal.draw(|f| {
            draw::draw_ui(f, app);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match handle_key(app, key)? {
                    UiAction::Continue => {}
                    UiAction::Exit => break,
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}
