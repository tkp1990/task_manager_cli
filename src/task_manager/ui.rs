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

fn log_ui_error(app: &mut App, context: &str, error: &dyn std::error::Error) {
    app.add_log("ERROR", &format!("{context}: {error}"));
}

fn highlighted_spans(text: &str, query: &str, base: Style, highlight: Style) -> Spans<'static> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Spans::from(Span::styled(text.to_string(), base));
    }

    let lower_text = text.to_lowercase();
    let lower_query = trimmed.to_lowercase();
    let mut spans = Vec::new();
    let mut cursor = 0;

    while let Some(relative) = lower_text[cursor..].find(&lower_query) {
        let start = cursor + relative;
        let end = start + lower_query.len();
        if start > cursor {
            spans.push(Span::styled(text[cursor..start].to_string(), base));
        }
        spans.push(Span::styled(text[start..end].to_string(), highlight));
        cursor = end;
    }

    if cursor < text.len() {
        spans.push(Span::styled(text[cursor..].to_string(), base));
    }

    Spans::from(spans)
}

fn task_status_spans(task: &crate::db::task_manager::models::Task) -> Spans<'static> {
    let mut spans = vec![Span::styled(
        if task.completed { "DONE " } else { "OPEN " },
        Style::default()
            .fg(if task.completed {
                Color::Green
            } else {
                Color::Yellow
            })
            .add_modifier(Modifier::BOLD),
    )];

    if task.favourite {
        spans.push(Span::styled(
            "STAR ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::styled(
        format!("Updated {}", task.updated_at),
        Style::default().fg(Color::DarkGray),
    ));

    Spans::from(spans)
}

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
    terminal.clear()?;
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
                        KeyCode::Char('p') => {
                            app.begin_task_presets();
                        }
                        KeyCode::Char('/') => {
                            app.begin_task_filter();
                        }
                        KeyCode::Char('W') => {
                            // Shift+W: open special topics popup
                            app.input_mode = InputMode::ViewingSpecialTopics;
                            app.special_tab_selected = 0;
                            if let Err(e) = app.load_special_tasks() {
                                app.input_mode = InputMode::Normal;
                                log_ui_error(app, "Failed to load special tasks", e.as_ref());
                            }
                        }
                        KeyCode::Char('a') => {
                            app.begin_add_task();
                        }
                        // Delete the selected task
                        KeyCode::Char('d') => {
                            app.begin_delete_task();
                        }
                        KeyCode::Char('e') => {
                            app.begin_edit_task();
                        }
                        // Toggle favourite flag for selected task
                        KeyCode::Char('f') => {
                            if let Err(e) = app.toggle_favourite() {
                                log_ui_error(app, "Failed to toggle favourite", e.as_ref());
                            }
                        }
                        // Toggle Help popup
                        KeyCode::Char('H') => {
                            app.input_mode = InputMode::Help;
                        }
                        KeyCode::Char('t') => {
                            if let Err(e) = app.toggle_task() {
                                log_ui_error(app, "Failed to toggle task", e.as_ref());
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
                            app.move_selection_down();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.move_selection_up();
                        }
                        // Switch topic to the left
                        KeyCode::Left | KeyCode::Char('h') => {
                            if app.selected_topic > 0 {
                                app.selected_topic -= 1;
                                if let Err(e) = app.load_tasks() {
                                    app.selected_topic += 1;
                                    log_ui_error(app, "Failed to load tasks", e.as_ref());
                                } else {
                                    app.selected = 0;
                                }
                            }
                        }
                        // Switch topic to the right
                        KeyCode::Right | KeyCode::Char('l') => {
                            if app.selected_topic < app.topics.len().saturating_sub(1) {
                                app.selected_topic += 1;
                                if let Err(e) = app.load_tasks() {
                                    app.selected_topic -= 1;
                                    log_ui_error(app, "Failed to load tasks", e.as_ref());
                                } else {
                                    app.selected = 0;
                                }
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
                            app.begin_add_topic();
                        }
                        // Delete current topic (except Favourites)
                        KeyCode::Char('X') => {
                            if !app.current_topic_is_special() {
                                if let Err(e) = app.delete_topic() {
                                    log_ui_error(app, "Failed to delete topic", e.as_ref());
                                }
                            }
                        }
                        _ => {}
                    },
                    InputMode::Filtering => match key.code {
                        KeyCode::Esc => {
                            app.clear_task_filter();
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Backspace => {
                            app.pop_task_filter_char();
                        }
                        KeyCode::Char(c) => {
                            app.append_task_filter_char(c);
                        }
                        _ => {}
                    },
                    InputMode::FilteringSpecial => match key.code {
                        KeyCode::Esc => {
                            app.clear_special_task_filter();
                            app.input_mode = InputMode::ViewingSpecialTopics;
                        }
                        KeyCode::Enter => {
                            app.input_mode = InputMode::ViewingSpecialTopics;
                        }
                        KeyCode::Backspace => {
                            app.pop_special_task_filter_char();
                        }
                        KeyCode::Char(c) => {
                            app.append_special_task_filter_char(c);
                        }
                        _ => {}
                    },
                    InputMode::PresetFilters => match key.code {
                        KeyCode::Esc => app.input_mode = InputMode::Normal,
                        KeyCode::Enter => {
                            app.apply_selected_task_preset();
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let len = app.task_filter_presets().len();
                            app.move_preset_down(len);
                        }
                        KeyCode::Up | KeyCode::Char('k') => app.move_preset_up(),
                        _ => {}
                    },
                    InputMode::PresetSpecialFilters => match key.code {
                        KeyCode::Esc => app.input_mode = InputMode::ViewingSpecialTopics,
                        KeyCode::Enter => {
                            app.apply_selected_special_task_preset();
                            app.input_mode = InputMode::ViewingSpecialTopics;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let len = app.task_filter_presets().len();
                            app.move_preset_down(len);
                        }
                        KeyCode::Up | KeyCode::Char('k') => app.move_preset_up(),
                        _ => {}
                    },
                    InputMode::AddingTaskName | InputMode::EditingTaskName => match key.code {
                        KeyCode::Enter => {
                            if !app.task_name_input.trim().is_empty() {
                                app.clear_task_form_message();
                                app.input_mode = if app.input_mode == InputMode::AddingTaskName {
                                    InputMode::AddingTaskDescription
                                } else {
                                    InputMode::EditingTaskDescription
                                };
                            } else {
                                app.set_task_form_message("Task name cannot be empty");
                            }
                        }
                        KeyCode::Esc => {
                            if app.input_mode == InputMode::AddingTaskName {
                                app.cancel_add_task();
                            } else {
                                app.reset_task_inputs();
                                app.input_mode = InputMode::Normal;
                            }
                        }
                        KeyCode::Char(c) => {
                            app.clear_task_form_message();
                            app.task_name_input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.clear_task_form_message();
                            app.task_name_input.pop();
                        }
                        _ => {}
                    },
                    InputMode::AddingTaskDescription | InputMode::EditingTaskDescription => {
                        match key.code {
                            KeyCode::Enter => {
                                if !app.task_name_input.trim().is_empty() {
                                    let name_clone = app.task_name_input.clone();
                                    let desc_clone = app.task_description_input.clone();
                                    let result =
                                        if app.input_mode == InputMode::AddingTaskDescription {
                                            app.add_task_with_details(&name_clone, &desc_clone)
                                        } else {
                                            app.edit_task(&name_clone, &desc_clone)
                                        };
                                    if let Err(e) = result {
                                        app.set_task_form_message(e.to_string());
                                        log_ui_error(
                                            app,
                                            if app.input_mode == InputMode::AddingTaskDescription {
                                                "Failed to add task"
                                            } else {
                                                "Failed to edit task"
                                            },
                                            e.as_ref(),
                                        );
                                    } else {
                                        app.add_log(
                                            "INFO",
                                            if app.input_mode == InputMode::AddingTaskDescription {
                                                "Task saved"
                                            } else {
                                                "Task updated"
                                            },
                                        );
                                        app.reset_task_inputs();
                                        app.input_mode = InputMode::Normal;
                                    }
                                } else {
                                    app.set_task_form_message("Task name cannot be empty");
                                }
                            }
                            KeyCode::Esc => {
                                if app.input_mode == InputMode::AddingTaskDescription {
                                    app.cancel_add_task();
                                } else {
                                    app.reset_task_inputs();
                                    app.input_mode = InputMode::Normal;
                                }
                            }
                            KeyCode::Tab => {
                                app.clear_task_form_message();
                                app.input_mode =
                                    if app.input_mode == InputMode::AddingTaskDescription {
                                        InputMode::AddingTaskName
                                    } else {
                                        InputMode::EditingTaskName
                                    };
                            }
                            KeyCode::Char(c) => {
                                app.clear_task_form_message();
                                app.task_description_input.push(c);
                            }
                            KeyCode::Backspace => {
                                app.clear_task_form_message();
                                app.task_description_input.pop();
                            }
                            _ => {}
                        }
                    }
                    InputMode::DeleteTask => match key.code {
                        KeyCode::Char('y') => {
                            if let Err(e) = app.delete_task() {
                                log_ui_error(app, "Failed to delete task", e.as_ref());
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                    InputMode::DeleteSpecialTask => match key.code {
                        KeyCode::Char('y') => {
                            if let Err(e) = app.delete_special_task() {
                                log_ui_error(app, "Failed to delete task", e.as_ref());
                            }
                            app.input_mode = InputMode::ViewingSpecialTopics;
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.input_mode = InputMode::ViewingSpecialTopics;
                        }
                        _ => {}
                    },
                    InputMode::AddingTopic => match key.code {
                        KeyCode::Enter => {
                            if !app.input.is_empty() {
                                let input_clone = app.input.clone();
                                if let Err(e) = app.add_topic(&input_clone) {
                                    log_ui_error(app, "Failed to add topic", e.as_ref());
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
                        KeyCode::Esc | KeyCode::Char('H') => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                    InputMode::ViewingSpecialTopics => match key.code {
                        KeyCode::Left | KeyCode::Char('h') => {
                            if app.special_tab_selected > 0 {
                                app.special_tab_selected -= 1;
                                app.special_task_selected = 0;
                                if let Err(e) = app.load_special_tasks() {
                                    app.special_tab_selected += 1;
                                    log_ui_error(app, "Failed to load special tasks", e.as_ref());
                                }
                            }
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            if app.special_tab_selected < 1 {
                                app.special_tab_selected += 1;
                                app.special_task_selected = 0;
                                if let Err(e) = app.load_special_tasks() {
                                    app.special_tab_selected -= 1;
                                    log_ui_error(app, "Failed to load special tasks", e.as_ref());
                                }
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.move_special_selection_up();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.move_special_selection_down();
                        }
                        KeyCode::Enter => {
                            // Toggle expansion of the selected task
                            let task_id = app
                                .get_current_special_tasks()
                                .get(app.special_task_selected)
                                .map(|task| task.id);
                            if let Some(id) = task_id {
                                if app.expanded.contains(&id) {
                                    app.expanded.remove(&id);
                                } else {
                                    app.expanded.insert(id);
                                }
                            }
                        }
                        KeyCode::Char('t') => {
                            if let Err(e) = app.toggle_special_task() {
                                log_ui_error(app, "Failed to toggle task", e.as_ref());
                            }
                        }
                        KeyCode::Char('f') => {
                            if let Err(e) = app.toggle_special_favourite() {
                                log_ui_error(app, "Failed to toggle favourite", e.as_ref());
                            }
                        }
                        KeyCode::Char('d') => {
                            app.begin_delete_special_task();
                        }
                        KeyCode::Char('p') => {
                            app.begin_special_task_presets();
                        }
                        KeyCode::Char('/') => {
                            app.begin_special_task_filter();
                        }
                        KeyCode::Esc => {
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
        .margin(0)
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
    let titles: Vec<Spans> = if app.topics.is_empty() {
        vec![Spans::from(Span::styled(
            "No topics. Press 'N' to add a topic.",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))]
    } else {
        app.topics
            .iter()
            .map(|t| Spans::from(Span::raw(&t.name)))
            .collect()
    };
    let tabs = Tabs::new(titles)
        .select(if app.topics.is_empty() {
            0
        } else {
            app.selected_topic
        })
        .block(Block::default().borders(Borders::ALL).title("Topics"))
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider(Span::raw("|"));
    f.render_widget(tabs, chunks[0]);

    // --- TASKS SECTION ---
    let filtered_indices = app.filtered_task_indices();
    let items: Vec<ListItem> = if app.tasks.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No tasks in this topic. Press 'a' to add a task.",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))])]
    } else if filtered_indices.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            format!("No tasks match \"{}\".", app.task_filter),
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))])]
    } else {
        filtered_indices
            .iter()
            .map(|task_index| {
                let task = &app.tasks[*task_index];
                let title_style = if task.completed {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::CROSSED_OUT)
                } else {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                };
                let summary = if task.description.trim().is_empty() {
                    "No description".to_string()
                } else if task.description.len() > 72 {
                    format!("{}...", &task.description[..72])
                } else {
                    task.description.clone()
                };
                let lines = if app.expanded.contains(&task.id) {
                    vec![
                        highlighted_spans(
                            &task.name,
                            &app.task_filter,
                            title_style,
                            title_style.bg(Color::Yellow).fg(Color::Black),
                        ),
                        highlighted_spans(
                            &summary,
                            &app.task_filter,
                            Style::default().fg(Color::Cyan),
                            Style::default().bg(Color::Yellow).fg(Color::Black),
                        ),
                        task_status_spans(task),
                        Spans::from(Span::styled(
                            format!(
                                "ID {} | Created {} | Topic {}",
                                task.id, task.created_at, task.topic_id
                            ),
                            Style::default().fg(Color::Gray),
                        )),
                    ]
                } else {
                    vec![
                        highlighted_spans(
                            &task.name,
                            &app.task_filter,
                            title_style,
                            title_style.bg(Color::Yellow).fg(Color::Black),
                        ),
                        highlighted_spans(
                            &summary,
                            &app.task_filter,
                            Style::default().fg(Color::Cyan),
                            Style::default().bg(Color::Yellow).fg(Color::Black),
                        ),
                        task_status_spans(task),
                    ]
                };
                ListItem::new(lines)
            })
            .collect()
    };

    let tasks_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if app.has_task_filter() {
                    format!("Tasks | Filter: {}", app.task_filter)
                } else {
                    "Tasks".to_string()
                }),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("=> ");

    let mut list_state = ListState::default();
    if !filtered_indices.is_empty() {
        list_state.select(
            filtered_indices
                .iter()
                .position(|index| *index == app.selected),
        );
    }
    f.render_stateful_widget(tasks_list, chunks[1], &mut list_state);

    // --- INSTRUCTIONS SECTION ---
    // In Normal mode, only show minimal instructions.
    let help_hint = Spans::from(vec![
        Span::raw("Press "),
        Span::styled("H", Style::default().fg(Color::Yellow)),
        Span::raw(" for help."),
    ]);
    let input_msg = match app.input_mode {
        InputMode::Filtering => Spans::from(vec![
            Span::raw("Filter tasks: "),
            Span::styled(app.task_filter.as_str(), Style::default().fg(Color::Yellow)),
            Span::raw(
                " (Enter to keep, Esc to clear. Tokens: status:, topic:, fav:, quotes, -negation)",
            ),
        ]),
        InputMode::AddingTopic => Spans::from(vec![
            Span::raw("Enter topic name (Press Enter to add, Esc to cancel): "),
            Span::raw(&app.input),
        ]),
        InputMode::AddingTaskName
        | InputMode::AddingTaskDescription
        | InputMode::EditingTaskName
        | InputMode::EditingTaskDescription => help_hint,
        _ => help_hint,
    };
    let help_message = Paragraph::new(input_msg)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Instructions"));
    f.render_widget(help_message, chunks[2]);

    // (Optional) Show current mode at the bottom.
    let mode_text = match app.input_mode {
        InputMode::Normal => "Normal Mode",
        InputMode::Filtering => "Filtering Tasks",
        InputMode::AddingTaskName => "Adding Task - Name Input",
        InputMode::AddingTaskDescription => "Adding Task - Description Input",
        InputMode::EditingTaskName => "Editing Task - Name Input",
        InputMode::EditingTaskDescription => "Editing Task - Description Input",
        InputMode::PresetFilters => "Task Presets",
        InputMode::PresetSpecialFilters => "Special Task Presets",
        InputMode::DeleteTask => "Delete Task",
        InputMode::DeleteSpecialTask => "Delete Task",
        InputMode::AddingTopic => "Adding Topic",
        InputMode::Help => "Viewing Help",
        InputMode::ViewingSpecialTopics => "Viewing Special Topics",
        InputMode::FilteringSpecial => "Filtering Special Tasks",
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
    if app.input_mode == InputMode::Help {
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
        || app.input_mode == InputMode::EditingTaskName
        || app.input_mode == InputMode::EditingTaskDescription
    {
        draw_add_task_popup(f, app);
    }

    if app.input_mode == InputMode::DeleteTask {
        draw_delete_popup(f, app);
    }

    if app.input_mode == InputMode::ViewingSpecialTopics
        || app.input_mode == InputMode::FilteringSpecial
        || app.input_mode == InputMode::PresetSpecialFilters
        || app.input_mode == InputMode::DeleteSpecialTask
    {
        draw_special_topics_popup(f, app);
    }

    if app.input_mode == InputMode::PresetFilters {
        draw_task_presets_popup(f, app, false);
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
            Constraint::Length(3), // Feedback
            Constraint::Length(3), // Instructions
        ])
        .split(popup_area);

    // Popup title
    let popup_title = Paragraph::new(
        if matches!(
            app.input_mode,
            InputMode::EditingTaskName | InputMode::EditingTaskDescription
        ) {
            "Edit Task"
        } else {
            "Create New Task"
        },
    )
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(popup_title, popup_layout[0]);

    // Name field
    let name_input_style = if matches!(
        app.input_mode,
        InputMode::AddingTaskName | InputMode::EditingTaskName
    ) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let name_input = Paragraph::new(app.task_name_input.as_ref())
        .style(name_input_style)
        .block(Block::default().borders(Borders::ALL).title("Task Name"));
    f.render_widget(name_input, popup_layout[1]);

    // Description field
    let desc_input_style = if matches!(
        app.input_mode,
        InputMode::AddingTaskDescription | InputMode::EditingTaskDescription
    ) {
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
        InputMode::EditingTaskName => "Edit task name and press Enter to continue. (Esc to cancel)",
        InputMode::EditingTaskDescription => {
            "Edit task description and press Enter to save. (Tab to edit name, Esc to cancel)"
        }
        _ => "",
    };

    let feedback_text = app
        .task_form_message
        .clone()
        .unwrap_or_else(|| "Enter a name, then a description, then save.".to_string());
    let feedback_style = if app.task_form_message.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let feedback = Paragraph::new(feedback_text)
        .style(feedback_style)
        .block(Block::default().borders(Borders::ALL).title("Feedback"));
    f.render_widget(feedback, popup_layout[4]);

    let instructions_text = Paragraph::new(instructions)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(instructions_text, popup_layout[5]);

    // Set cursor position based on input mode
    if matches!(
        app.input_mode,
        InputMode::AddingTaskName | InputMode::EditingTaskName
    ) {
        // Set cursor to end of name input
        f.set_cursor(
            popup_layout[1].x + app.task_name_input.len() as u16 + 1,
            popup_layout[1].y + 1,
        );
    } else if matches!(
        app.input_mode,
        InputMode::AddingTaskDescription | InputMode::EditingTaskDescription
    ) {
        // Set cursor to end of description input (note: doesn't handle wrapping)
        f.set_cursor(
            popup_layout[3].x + app.task_description_input.len() as u16 + 1,
            popup_layout[3].y + 1,
        );
    }
}

fn draw_delete_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Create a nicely sized popup for task deletion confirmation
    let size = f.size();
    let delete_popup_area = centered_rect(50, 20, size);
    f.render_widget(Clear, delete_popup_area); // Clear the area first

    // Split the popup into sections
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Message
            Constraint::Length(3), // Instructions
        ])
        .split(delete_popup_area);

    // Popup title
    let popup_title = Paragraph::new("Delete Confirmation")
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(tui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(popup_title, popup_layout[0]);

    // Task details to be deleted
    let task_name = if let Some(task) = app.tasks.get(app.selected) {
        &task.name
    } else {
        "Unknown Task"
    };

    let delete_message = Paragraph::new(format!(
        "Are you sure you want to delete \"{}\"?",
        task_name
    ))
    .style(Style::default().fg(Color::Red))
    .alignment(tui::layout::Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(delete_message, popup_layout[1]);

    // Instructions
    let instructions = Paragraph::new("Press [Y] to confirm deletion or [N] to cancel")
        .style(Style::default().fg(Color::Cyan))
        .alignment(tui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(instructions, popup_layout[2]);
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
        build_help_line(
            "Filter Tasks:",
            "'/'",
            "filter live. Supports status:done, topic:work, fav:true, quoted phrases, and -negation.",
        ),
        build_help_line(
            "Task Presets:",
            "'p'",
            "open saved preset filters for quick reuse.",
        ),
        build_help_line(
            "Edit Task:",
            "'e'",
            "opens the same two-field form used for task creation.",
        ),
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
        build_help_line(
            "Open Favourites/Completed:",
            "Shift+W",
            "open a floating window with Favourites and Completed tabs.",
        ),
        build_help_line(
            "Switch Special Tabs:",
            "Left/Right or h/l",
            "switch between Favourites and Completed in the popup.",
        ),
        build_help_line(
            "Navigate Special Tasks:",
            "Up/Down or j/k",
            "navigate tasks in the special popup.",
        ),
        build_help_line(
            "Special Popup Actions:",
            "t/f/d/Enter",
            "toggle complete/favourite, delete, or expand in popup.",
        ),
        build_help_line(
            "Close Popup:",
            "Esc",
            "close the Favourites/Completed window.",
        ),
        build_help_line("Toggle Help:", "'H'", "to show/hide help."),
        build_help_line("Quit:", "'q'", "to exit the application."),
    ]
}

fn draw_special_topics_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let popup_area = centered_rect(70, 70, size);
    f.render_widget(Clear, popup_area);
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tabs
            Constraint::Min(5),    // Task list
            Constraint::Length(3), // Instructions
        ])
        .split(popup_area);
    // Tabs
    let tab_titles = vec![Spans::from("Favourites"), Spans::from("Completed")];
    let tabs = Tabs::new(tab_titles)
        .select(app.special_tab_selected)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Special Tasks"),
        )
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider(Span::raw("|"));
    f.render_widget(tabs, popup_layout[0]);

    // Task list
    let tasks = app.get_current_special_tasks();
    let filtered_indices = app.filtered_special_task_indices();
    let items: Vec<ListItem> = if tasks.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No tasks found.",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))])]
    } else if filtered_indices.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            format!("No tasks match \"{}\".", app.special_task_filter),
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))])]
    } else {
        filtered_indices
            .iter()
            .map(|task_index| {
                let task = &tasks[*task_index];
                let description_style = if task.completed {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                // If task is expanded, show extra details
                let lines = if app.expanded.contains(&task.id) {
                    vec![
                        highlighted_spans(
                            &task.name,
                            &app.special_task_filter,
                            description_style,
                            description_style.bg(Color::Yellow).fg(Color::Black),
                        ),
                        highlighted_spans(
                            &format!("Description: {}", task.description),
                            &app.special_task_filter,
                            description_style,
                            Style::default().bg(Color::Yellow).fg(Color::Black),
                        ),
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
                    vec![highlighted_spans(
                        &format!("{}: {}", task.name, task.description),
                        &app.special_task_filter,
                        description_style,
                        Style::default().bg(Color::Yellow).fg(Color::Black),
                    )]
                };
                ListItem::new(lines)
            })
            .collect()
    };

    let tasks_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if app.has_special_task_filter() {
                    format!("Tasks | Filter: {}", app.special_task_filter)
                } else {
                    "Tasks".to_string()
                }),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("=> ");

    let mut list_state = ListState::default();
    if !filtered_indices.is_empty() {
        list_state.select(
            filtered_indices
                .iter()
                .position(|index| *index == app.special_task_selected),
        );
    }
    f.render_stateful_widget(tasks_list, popup_layout[1], &mut list_state);

    // Instructions
    let instructions = if app.input_mode == InputMode::DeleteSpecialTask {
        "Press [Y] to confirm deletion or [N] to cancel"
    } else if app.input_mode == InputMode::PresetSpecialFilters {
        "Preset filters. Enter to apply, Esc to cancel"
    } else if app.input_mode == InputMode::FilteringSpecial {
        "Filter special tasks. Enter to keep filter, Esc to clear"
    } else {
        "Up/Down: Navigate | /: Filter | p: Presets | Enter: Expand | t: Toggle | f: Favourite | d: Delete | Esc: Close"
    };
    let instructions_text = Paragraph::new(instructions)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title("Instructions"));
    f.render_widget(instructions_text, popup_layout[2]);

    // Show delete popup if needed
    if app.input_mode == InputMode::DeleteSpecialTask {
        let tasks = app.get_current_special_tasks();
        if let Some(task) = tasks.get(app.special_task_selected) {
            let delete_area = centered_rect(50, 20, size);
            f.render_widget(Clear, delete_area);
            let delete_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(delete_area);

            let delete_title = Paragraph::new("Delete Confirmation")
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(tui::layout::Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(delete_title, delete_layout[0]);

            let delete_msg = Paragraph::new(format!(
                "Are you sure you want to delete \"{}\"?",
                task.name
            ))
            .style(Style::default().fg(Color::Red))
            .alignment(tui::layout::Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(delete_msg, delete_layout[1]);

            let delete_instructions = Paragraph::new("Press [Y] to confirm or [N] to cancel")
                .style(Style::default().fg(Color::Cyan))
                .alignment(tui::layout::Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(delete_instructions, delete_layout[2]);
        }
    }

    if app.input_mode == InputMode::PresetSpecialFilters {
        draw_task_presets_popup(f, app, true);
    }
}

fn draw_task_presets_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, special: bool) {
    let size = f.size();
    let popup_area = centered_rect(55, 45, size);
    f.render_widget(Clear, popup_area);

    let presets = app.task_filter_presets();
    let items: Vec<ListItem> = presets
        .iter()
        .map(|(name, query)| {
            ListItem::new(vec![
                Spans::from(Span::styled(
                    (*name).to_string(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Spans::from(Span::styled(
                    (*query).to_string(),
                    Style::default().fg(Color::Gray),
                )),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(if special {
            "Special Task Presets"
        } else {
            "Task Presets"
        }))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    state.select(Some(app.preset_selected));
    f.render_stateful_widget(list, popup_area, &mut state);
}
