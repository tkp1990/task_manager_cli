use crate::task_manager::app::{App, InputMode};
use crossterm::event::{self, Event, KeyCode};
use std::time::Duration;
use std::time::Instant;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs},
    Frame, Terminal,
};

/// Helper function to create a centered rectangle with the given percentage size.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);
    let vertical = popup_layout[1];
    let popup_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(vertical);
    popup_layout[1]
}

pub fn run<B: Backend>(
    mut app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<(), Box<dyn std::error::Error>> {
    // --- EVENT LOOP ---
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| {
            draw_ui(f, &mut app);
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
                        // Delete the selected task
                        KeyCode::Char('d') => {
                            if let Err(e) = app.delete_task() {
                                eprintln!("Error deleting task: {:?}", e);
                                app.add_log("ERROR", "Failed to delete task");
                            }
                        }
                        KeyCode::Char('e') => {
                            app.input_mode = InputMode::EditingTask;
                            app.input.clear();
                        }
                        // Toggle favourite flag for selected task
                        KeyCode::Char('f') => {
                            if let Err(e) = app.toggle_favourite() {
                                eprintln!("Error toggling favourite: {:?}", e);
                                app.add_log("ERROR", "Failed to toggle favourite");
                            }
                        }
                        // Toggle Help popup
                        KeyCode::Char('H') => {
                            app.input_mode = InputMode::Help;
                            app.show_help = !app.show_help;
                        }
                        KeyCode::Char('t') => {
                            if let Err(e) = app.toggle_task() {
                                eprintln!("Error toggling task: {:?}", e);
                                app.add_log("ERROR", "Failed to toggle task");
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
                    InputMode::Help => match key.code {
                        KeyCode::Esc => {
                            app.show_help = false;
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char('H') => {
                            app.show_help = false;
                            app.input_mode = InputMode::Normal;
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

    Ok(())
}

pub fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Define the layout.
    let size = f.size();
    // Split the screen into five sections:
    // 1. Topics (Tabs), 2. Tasks list, 3. Instructions, 4. Mode, 5. Logs.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),  // Topics Tabs
                Constraint::Min(5),     // Task list
                Constraint::Length(3),  // Instructions
                Constraint::Length(3),  // Mode indicator
                Constraint::Length(15), // Logs section
            ]
            .as_ref(),
        )
        .split(size);

    // --- TOPICS SECTION (Tabs) ---
    let titles: Vec<Spans> = app
        .topics
        .iter()
        .map(|t| Spans::from(Span::raw(&t.name)))
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.selected_topic)
        .block(Block::default().borders(Borders::ALL).title("Topics"))
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider(Span::raw("|"));
    f.render_widget(tabs, chunks[0]);

    // --- TASKS SECTION ---
    let items: Vec<ListItem> = app
        .tasks
        .iter()
        .map(|task| {
            // If task is expanded, show extra details; otherwise, only show the task name.
            let lines = if app.expanded.contains(&task.id) {
                vec![
                    Spans::from(Span::raw(format!("{}", task.description))),
                    Spans::from(Span::styled(
                        format!(
                            "ID: {} | Completed: {} | Favourite: {} | Created: {} | Updated: {}",
                            task.id,
                            if task.completed { "Yes" } else { "No" },
                            if task.favourite { "Yes" } else { "No" },
                            task.created_at,
                            task.updated_at
                        ),
                        Style::default().fg(Color::Gray),
                    )),
                ]
            } else {
                vec![Spans::from(Span::raw(format!("{}", task.description)))]
            };
            ListItem::new(lines)
        })
        .collect();

    let tasks_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Tasks"))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected));
    f.render_stateful_widget(tasks_list, chunks[1], &mut list_state);

    // --- INSTRUCTIONS SECTION ---
    // In Normal mode, only show minimal instructions.
    let help_hint = Spans::from(vec![
        Span::raw("Press "),
        Span::styled("H", Style::default().fg(Color::Yellow)),
        Span::raw(" for help."),
    ]);
    let input_msg = match app.input_mode {
        InputMode::AddingTask => Spans::from(vec![
            Span::raw("Enter task description (Press Enter to add, Esc to cancel): "),
            Span::raw(&app.input),
        ]),
        InputMode::AddingTopic => Spans::from(vec![
            Span::raw("Enter topic name (Press Enter to add, Esc to cancel): "),
            Span::raw(&app.input),
        ]),
        InputMode::EditingTask => Spans::from(vec![
            Span::raw("Edit task description (Press Enter to save, Esc to cancel): "),
            Span::raw(&app.input),
        ]),
        _ => help_hint,
    };
    let help_message = Paragraph::new(input_msg)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Instructions"));
    f.render_widget(help_message, chunks[2]);

    // (Optional) Show current mode at the bottom.
    let mode_text = match app.input_mode {
        InputMode::Normal => "Normal Mode",
        InputMode::AddingTask => "Add Mode",
        InputMode::EditingTask => "Editing Mode",
        InputMode::AddingTopic => "Adding Topic",
        InputMode::Help => "Viewing Help",
    };
    let mode =
        Paragraph::new(mode_text).block(Block::default().borders(Borders::ALL).title("Mode"));
    f.render_widget(mode, chunks[3]);

    // --- LOGS SECTION ---
    // Determine how many lines can be shown.
    let log_area_height = chunks[4].height as usize;
    let total_logs = app.logs.len();
    // Compute the starting index, ensuring we don't underflow.
    let start = if total_logs > log_area_height + app.log_offset {
        total_logs - log_area_height - app.log_offset
    } else {
        0
    };
    let visible_logs: Vec<ListItem> = app.logs[start..]
        .iter()
        .map(|line| ListItem::new(Span::raw(line)))
        .collect();
    let logs_list =
        List::new(visible_logs).block(Block::default().borders(Borders::ALL).title("Logs"));
    f.render_widget(logs_list, chunks[4]);

    // --- HELP POPUP (if enabled) ---
    if app.show_help {
        let help_text = vec![
            Spans::from(Span::styled("Help - Available Operations", Style::default().add_modifier(Modifier::BOLD))),
            Spans::from(""),
            Spans::from(Span::raw("Add Task: Press 'a' to add a new task (in a non-Favourites topic). Type the description and press Enter.")),
            Spans::from(Span::raw("Add Task: Press 'e' to edit a task. Edit the description and press Enter.")),
            Spans::from(Span::raw("Toggle Complete: Press 't' to mark a task complete/incomplete.")),
            Spans::from(Span::raw("Toggle Favourite: Press 'f' to mark/unmark a task as favourite.")),
            Spans::from(Span::raw("Delete Task: Press 'd' to delete the selected task.")),
            Spans::from(Span::raw("Expand/Collapse Task: Press Enter on a task to show/hide details.")),
            Spans::from(Span::raw("Navigating Tasks: Use Up/Down arrow keys or j/k to change topics.")),
            Spans::from(Span::raw("Switch Topics: Use Left/Right arrow keys or h/l to change topics.")),
            Spans::from(Span::raw("Add Topic: Press 'N' to add a new topic. Then enter the topic name and press Enter.")),
            Spans::from(Span::raw("Delete Topic: Press 'X' to delete the current topic (Favourites is protected).")),
            Spans::from(Span::raw("Scroll Logs: Use PageUp/PageDown to scroll through logs.")),
            Spans::from(Span::raw("Toggle Help: Press 'H' to hide this help window.")),
            Spans::from(Span::raw("Quit: Press 'q' to exit the application.")),
        ];
        let help_paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(tui::widgets::Wrap { trim: true });
        let area = centered_rect(60, 70, size);
        // Clear the background behind the popup
        f.render_widget(Clear, area);
        f.render_widget(help_paragraph, area);
    }
}
