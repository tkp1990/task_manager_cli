use crossterm::event::{self, Event, KeyEvent};
use std::{
    error::Error,
    time::{Duration, Instant},
};
use tui::{backend::Backend, Frame, Terminal};

pub fn run_event_loop<B, State, Action, Draw, Handle, Process>(
    terminal: &mut Terminal<B>,
    state: &mut State,
    mut draw: Draw,
    mut handle_key: Handle,
    mut process_action: Process,
) -> Result<(), Box<dyn Error>>
where
    B: Backend,
    Draw: FnMut(&mut Frame<B>, &mut State),
    Handle: FnMut(&mut State, KeyEvent) -> Result<Action, Box<dyn Error>>,
    Process: FnMut(Action, &mut State, &mut Terminal<B>) -> Result<bool, Box<dyn Error>>,
{
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    terminal.clear()?;

    loop {
        terminal.draw(|f| draw(f, state))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                let action = handle_key(state, key)?;
                if process_action(action, state, terminal)? {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}
