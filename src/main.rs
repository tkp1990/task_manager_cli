mod app;
mod ui;
use app::{App, InputMode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use tui::{backend::CrosstermBackend, Terminal};

fn main() -> Result<(), Box<dyn Error>> {
    // Create our App instance (which sets up the DB and loads tasks)
    let mut app = App::new("task_manager.db")?;

    // --- SETUP TERMINAL ---
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // --- EVENT LOOP ---
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| {
            ui::draw_ui(f, &mut app);
        })?;

        // Poll for input or tick.
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('a') => {
                            if app.current_topic_is_favourites() {
                                // You may show a log message if desired.
                            } else {
                                app.input_mode = InputMode::AddingTask;
                                app.input.clear();
                            }
                        }
                        KeyCode::Char('e') => {
                            app.input_mode = InputMode::EditingTask;
                            app.input.clear();
                        }
                        KeyCode::Char('t') => {
                            if let Err(e) = app.toggle_task() {
                                eprintln!("Error toggling task: {:?}", e);
                                app.add_log("ERROR", "Failed to toggle task");
                            }
                        }
                        // Delete the selected task
                        KeyCode::Char('d') => {
                            if let Err(e) = app.delete_task() {
                                eprintln!("Error deleting task: {:?}", e);
                                app.add_log("ERROR", "Failed to delete task");
                            }
                        }
                        // Toggle favourite flag for selected task
                        KeyCode::Char('f') => {
                            if let Err(e) = app.toggle_favourite() {
                                eprintln!("Error toggling favourite: {:?}", e);
                                app.add_log("ERROR", "Failed to toggle favourite");
                            }
                        }
                        KeyCode::Enter => {
                            // Toggle expansion of the selected task.
                            if let Some(task) = app.tasks.get(app.selected) {
                                if app.expanded.contains(&task.id) {
                                    app.expanded.remove(&task.id);
                                } else {
                                    app.expanded.insert(task.id);
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if app.selected < app.tasks.len().saturating_sub(1) {
                                app.selected += 1;
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if app.selected > 0 {
                                app.selected -= 1;
                            }
                        }
                        // Switch topic to the left
                        KeyCode::Left | KeyCode::Char('h') => {
                            if app.selected_topic > 0 {
                                app.selected_topic -= 1;
                                app.load_tasks().unwrap();
                                app.selected = 0;
                            }
                        }
                        // Switch topic to the right
                        KeyCode::Right | KeyCode::Char('l') => {
                            if app.selected_topic < app.topics.len().saturating_sub(1) {
                                app.selected_topic += 1;
                                app.load_tasks().unwrap();
                                app.selected = 0;
                            }
                        }
                        KeyCode::PageUp => {
                            app.log_offset += 1;
                        }
                        KeyCode::PageDown => {
                            if app.log_offset > 0 {
                                app.log_offset -= 1;
                            }
                        }
                        // Add a new topic
                        KeyCode::Char('N') => {
                            app.input_mode = InputMode::AddingTopic;
                            app.input.clear();
                        }
                        // Delete current topic (except Favourites)
                        KeyCode::Char('X') => {
                            if !app.current_topic_is_favourites() {
                                if let Err(e) = app.delete_topic() {
                                    eprintln!("Error deleting topic: {:?}", e);
                                    app.add_log("ERROR", "Failed to delete topic");
                                }
                            }
                        }
                        _ => {}
                    },
                    InputMode::AddingTask => match key.code {
                        KeyCode::Enter => {
                            // Add the task and switch back to normal mode.
                            if !app.input.is_empty() {
                                let input_clone = app.input.clone();
                                if let Err(e) = app.add_task(&input_clone) {
                                    eprintln!("Error adding task: {:?}", e);
                                    app.add_log("ERROR", "Failed to add task");
                                    app.add_log("ERROR", &format!("{:?}", e));
                                }
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                    InputMode::EditingTask => match key.code {
                        KeyCode::Enter => {
                            // Add the task and switch back to normal mode.
                            if !app.input.is_empty() {
                                let input_clone = app.input.clone();
                                if let Err(e) = app.edit_task(&input_clone) {
                                    eprintln!("Error editing task: {:?}", e);
                                    app.add_log("ERROR", "Failed to edit task");
                                }
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                    InputMode::AddingTopic => match key.code {
                        KeyCode::Enter => {
                            if !app.input.is_empty() {
                                let input_clone = app.input.clone();
                                if let Err(e) = app.add_topic(&input_clone) {
                                    eprintln!("Error adding topic: {:?}", e);
                                    app.add_log("ERROR", "Failed to add topic");
                                } else {
                                    app.add_log("INFO", &format!("Added topic: {}", app.input));
                                }
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // --- CLEANUP ---
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
