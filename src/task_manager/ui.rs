use crate::task_manager::app::{App, InputMode};
use crossterm::event::{self, Event, KeyCode};
use std::time::Duration;
use std::time::Instant;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
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
                                app.input_mode = InputMode::AddingTaskName;
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
                    InputMode::AddingTaskName => match key.code {
                        KeyCode::Enter => {
                            if !app.task_name_input.is_empty() {
                                // Move to description input
                                app.input_mode = InputMode::AddingTaskDescription;
                            }
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            app.task_name_input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.task_name_input.pop();
                        }
                        _ => {}
                    },
                    InputMode::AddingTaskDescription => match key.code {
                        KeyCode::Enter => {
                            if !app.task_name_input.is_empty() {
                                // Save the task with name and description
                                let name_clone = app.task_name_input.clone();
                                let desc_clone = app.task_description_input.clone();
                                if let Err(e) = app.add_task_with_details(&name_clone, &desc_clone)
                                {
                                    eprintln!("Error adding task: {:?}", e);
                                    app.add_log("ERROR", "Failed to add task");
                                    app.add_log("ERROR", &format!("{:?}", e));
                                }
                                app.reset_task_inputs();
                                app.input_mode = InputMode::Normal;
                            }
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Tab => {
                            // Let Tab key switch back to name field
                            app.input_mode = InputMode::AddingTaskName;
                        }
                        KeyCode::Char(c) => {
                            app.task_description_input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.task_description_input.pop();
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
            // Determine the text style based on completion status.
            let description_style = if task.completed {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Cyan)
            };
            // If task is expanded, show extra details; otherwise, only show the task name.
            let lines = if app.expanded.contains(&task.id) {
                vec![
                    Spans::from(Span::styled(format!("{}", task.name), description_style)),
                    Spans::from(Span::styled(
                        format!("Description: {}", task.description),
                        description_style,
                    )),
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
                vec![Spans::from(Span::styled(
                    format!("{}", task.name),
                    description_style,
                ))]
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
        .highlight_symbol("=> ");

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
        InputMode::AddingTaskName | InputMode::AddingTaskDescription => help_hint,
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
        InputMode::AddingTaskName => "Adding Task - Name Input",
        InputMode::AddingTaskDescription => "Adding Task - Description Input",
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
        let help_text = get_help_text();
        let help_paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .wrap(Wrap { trim: true });
        let area = centered_rect(60, 70, size);
        // Clear the background behind the popup before rendering help
        f.render_widget(Clear, area);
        f.render_widget(help_paragraph, area);
    }

    if app.input_mode == InputMode::AddingTaskName
        || app.input_mode == InputMode::AddingTaskDescription
    {
        draw_add_task_popup(f, app);
    }
}

fn draw_add_task_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Create a popup for task creation
    let size = f.size();
    let popup_area = centered_rect(60, 40, size);
    f.render_widget(Clear, popup_area); // Clear the area first

    // Split the popup into different sections
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Name field
            Constraint::Length(1), // Spacer
            Constraint::Min(5),    // Description field
            Constraint::Length(3), // Instructions
        ])
        .split(popup_area);

    // Popup title
    let popup_title = Paragraph::new("Create New Task")
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(popup_title, popup_layout[0]);

    // Name field
    let name_input_style = if app.input_mode == InputMode::AddingTaskName {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let name_input = Paragraph::new(app.task_name_input.as_ref())
        .style(name_input_style)
        .block(Block::default().borders(Borders::ALL).title("Task Name"));
    f.render_widget(name_input, popup_layout[1]);

    // Description field
    let desc_input_style = if app.input_mode == InputMode::AddingTaskDescription {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let desc_input = Paragraph::new(app.task_description_input.as_ref())
        .style(desc_input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Task Description"),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(desc_input, popup_layout[3]);

    // Instructions
    let instructions = match app.input_mode {
        InputMode::AddingTaskName => "Enter task name and press Enter to continue. (Esc to cancel)",
        InputMode::AddingTaskDescription => {
            "Enter task description and press Enter to save. (Tab to edit name, Esc to cancel)"
        }
        _ => "",
    };

    let instructions_text = Paragraph::new(instructions)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(instructions_text, popup_layout[4]);

    // Set cursor position based on input mode
    if app.input_mode == InputMode::AddingTaskName {
        // Set cursor to end of name input
        f.set_cursor(
            popup_layout[1].x + app.task_name_input.len() as u16 + 1,
            popup_layout[1].y + 1,
        );
    } else if app.input_mode == InputMode::AddingTaskDescription {
        // Set cursor to end of description input (note: doesn't handle wrapping)
        f.set_cursor(
            popup_layout[3].x + app.task_description_input.len() as u16 + 1,
            popup_layout[3].y + 1,
        );
    }
}

/// Build a single help line with a title, key command, and description.
fn build_help_line(
    title: &'static str,
    key: &'static str,
    description: &'static str,
) -> Spans<'static> {
    Spans::from(vec![
        Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Press "),
        Span::styled(
            key,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::raw(description),
    ])
}

/// Returns the help text as a vector of Spans.
pub fn get_help_text() -> Vec<Spans<'static>> {
    vec![
        Spans::from(Span::styled(
            "Help - Available Operations",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from(""),
        build_help_line(
            "Add Task:",
            "'a'",
            "opens a popup to create a new task with name and description.",
        ),
        build_help_line("Edit Task:", "'e'", "to edit an existing task."),
        build_help_line(
            "Toggle Complete:",
            "'t'",
            "to mark a task complete/incomplete.",
        ),
        build_help_line("Toggle Favourite:", "'f'", "to mark/unmark as favourite."),
        build_help_line("Delete Task:", "'d'", "to delete the selected task."),
        build_help_line("Expand/Collapse Task:", "Enter", "to toggle details."),
        build_help_line(
            "Navigate Tasks:",
            "Up/Down or j/k",
            "to move between tasks.",
        ),
        build_help_line("Switch Topics:", "Left/Right or h/l", "to change topics."),
        build_help_line("Add Topic:", "'N'", "to add a new topic."),
        build_help_line(
            "Delete Topic:",
            "'X'",
            "to delete the current topic (Favourites is protected).",
        ),
        build_help_line("Scroll Logs:", "PageUp/PageDown", "to scroll logs."),
        build_help_line("Toggle Help:", "Ctrl+h", "to hide help."),
        build_help_line("Quit:", "'q'", "to exit the application."),
    ]
}
