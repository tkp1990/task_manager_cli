use crate::task_manager::app::{App, InputMode};
use crate::ui_style::{self, Accent, PopupSize};
use crossterm::event::{self, Event, KeyCode};
use std::time::Duration;
use std::time::Instant;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};

#[derive(Clone, Copy)]
struct PaletteCommand {
    id: &'static str,
    shortcut: &'static str,
    group: &'static str,
    label: &'static str,
    description: &'static str,
    keywords: &'static str,
}

fn log_ui_error(app: &mut App, context: &str, error: &dyn std::error::Error) {
    app.add_log("ERROR", &format!("{context}: {error}"));
}

fn palette_matches(command: &PaletteCommand, query: &str) -> bool {
    let trimmed = query.trim().to_lowercase();
    trimmed.is_empty()
        || command.label.to_lowercase().contains(&trimmed)
        || command.description.to_lowercase().contains(&trimmed)
        || command.keywords.to_lowercase().contains(&trimmed)
}

fn task_palette_commands(app: &App) -> Vec<PaletteCommand> {
    match app.command_palette_return_mode {
        InputMode::ViewingSpecialTopics
        | InputMode::FilteringSpecial
        | InputMode::PresetSpecialFilters
        | InputMode::DeleteSpecialTask => vec![
            PaletteCommand {
                id: "close_special",
                shortcut: "Esc",
                group: "Special",
                label: "Close Special Tasks",
                description: "Return to the main task list.",
                keywords: "close special popup return",
            },
            PaletteCommand {
                id: "filter_special",
                shortcut: "/",
                group: "Special",
                label: "Filter Special Tasks",
                description: "Search favourites or completed tasks.",
                keywords: "filter search special favourites completed",
            },
            PaletteCommand {
                id: "special_presets",
                shortcut: "p",
                group: "Special",
                label: "Open Special Presets",
                description: "Apply or manage saved special-task filters.",
                keywords: "presets saved filters special",
            },
            PaletteCommand {
                id: "help",
                shortcut: "H",
                group: "General",
                label: "Open Help",
                description: "Show task manager shortcuts and modes.",
                keywords: "help shortcuts docs",
            },
        ],
        _ => vec![
            PaletteCommand {
                id: "add_task",
                shortcut: "a",
                group: "Create",
                label: "Add Task",
                description: "Create a new task in the current topic.",
                keywords: "new task create a",
            },
            PaletteCommand {
                id: "add_topic",
                shortcut: "N",
                group: "Create",
                label: "Add Topic",
                description: "Create a new topic tab.",
                keywords: "new topic create category",
            },
            PaletteCommand {
                id: "edit_task",
                shortcut: "e",
                group: "Edit",
                label: "Edit Task",
                description: "Rename or update the selected task.",
                keywords: "edit rename update selected",
            },
            PaletteCommand {
                id: "delete_task",
                shortcut: "d",
                group: "Edit",
                label: "Delete Task",
                description: "Delete the selected task.",
                keywords: "delete remove task",
            },
            PaletteCommand {
                id: "toggle_done",
                shortcut: "t",
                group: "State",
                label: "Toggle Complete",
                description: "Mark the selected task open or done.",
                keywords: "complete done toggle t space",
            },
            PaletteCommand {
                id: "toggle_favourite",
                shortcut: "f",
                group: "State",
                label: "Toggle Favourite",
                description: "Star or unstar the selected task.",
                keywords: "favorite favourite star f",
            },
            PaletteCommand {
                id: "filter_tasks",
                shortcut: "/",
                group: "Discover",
                label: "Filter Tasks",
                description: "Search tasks by status, topic, or favourite.",
                keywords: "filter search status topic fav",
            },
            PaletteCommand {
                id: "open_presets",
                shortcut: "p",
                group: "Discover",
                label: "Open Presets",
                description: "Apply or manage saved task filters.",
                keywords: "presets saved filters p",
            },
            PaletteCommand {
                id: "open_special",
                shortcut: "W",
                group: "Discover",
                label: "Open Special Tasks",
                description: "Browse favourites and completed tasks.",
                keywords: "special favourites completed popup w",
            },
            PaletteCommand {
                id: "help",
                shortcut: "H",
                group: "General",
                label: "Open Help",
                description: "Show task manager shortcuts and modes.",
                keywords: "help shortcuts docs",
            },
        ],
    }
}

fn visible_task_palette_commands(app: &App) -> Vec<PaletteCommand> {
    let query = app.command_palette_query.trim().to_lowercase();
    let mut commands = task_palette_commands(app)
        .into_iter()
        .filter(|command| palette_matches(command, &app.command_palette_query))
        .collect::<Vec<_>>();
    commands.sort_by_key(|command| {
        let recent_rank = app
            .recent_palette_commands
            .iter()
            .position(|item| item == command.id)
            .unwrap_or(usize::MAX);
        let label = command.label.to_lowercase();
        let keyword_hit = command.keywords.to_lowercase().contains(&query);
        let prefix = !query.is_empty() && label.starts_with(&query);
        (
            recent_rank,
            !prefix,
            !keyword_hit,
            command.group,
            command.label,
        )
    });
    commands
}

fn execute_task_palette_command(
    app: &mut App,
    command_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match command_id {
        "add_task" => app.begin_add_task(),
        "add_topic" => app.begin_add_topic(),
        "edit_task" => app.begin_edit_task(),
        "delete_task" => app.begin_delete_task(),
        "toggle_done" => app.toggle_task()?,
        "toggle_favourite" => app.toggle_favourite()?,
        "filter_tasks" => app.begin_task_filter(),
        "open_presets" => app.begin_task_presets(),
        "open_special" => {
            app.input_mode = InputMode::ViewingSpecialTopics;
            app.special_tab_selected = 0;
            app.load_special_tasks()?;
        }
        "close_special" => app.input_mode = InputMode::Normal,
        "filter_special" => app.begin_special_task_filter(),
        "special_presets" => app.begin_special_task_presets(),
        "help" => app.input_mode = InputMode::Help,
        _ => {}
    }
    app.record_palette_command(command_id)?;
    Ok(())
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
        if task.completed {
            ui_style::success_style()
        } else {
            ui_style::warning_style()
        },
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
        ui_style::subtle_style(),
    ));

    Spans::from(spans)
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
                        KeyCode::Char(':') => {
                            app.begin_command_palette();
                        }
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
                    InputMode::CommandPalette => match key.code {
                        KeyCode::Esc => {
                            app.close_command_palette();
                        }
                        KeyCode::Enter => {
                            if let Some(command) = visible_task_palette_commands(app)
                                .get(app.command_palette_selected)
                                .copied()
                            {
                                app.close_command_palette();
                                if let Err(e) = execute_task_palette_command(app, command.id) {
                                    log_ui_error(
                                        app,
                                        "Failed to execute palette command",
                                        e.as_ref(),
                                    );
                                }
                            } else {
                                app.close_command_palette();
                            }
                        }
                        KeyCode::Backspace => {
                            app.command_palette_query.pop();
                            app.command_palette_selected = 0;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if app.command_palette_selected > 0 {
                                app.command_palette_selected -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let visible = visible_task_palette_commands(app);
                            if app.command_palette_selected + 1 < visible.len() {
                                app.command_palette_selected += 1;
                            }
                        }
                        KeyCode::Char(c) => {
                            app.command_palette_query.push(c);
                            app.command_palette_selected = 0;
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
                        KeyCode::Char('S') => app.begin_save_task_preset(),
                        KeyCode::Char('x') => {
                            if let Err(e) = app.delete_selected_task_preset() {
                                log_ui_error(app, "Failed to delete task preset", e.as_ref());
                            }
                        }
                        KeyCode::Enter => {
                            app.apply_selected_task_preset();
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let len = app.all_task_filter_presets().len();
                            app.move_preset_down(len);
                        }
                        KeyCode::Up | KeyCode::Char('k') => app.move_preset_up(),
                        _ => {}
                    },
                    InputMode::PresetSpecialFilters => match key.code {
                        KeyCode::Esc => app.input_mode = InputMode::ViewingSpecialTopics,
                        KeyCode::Char('S') => app.begin_save_special_task_preset(),
                        KeyCode::Char('x') => {
                            if let Err(e) = app.delete_selected_task_preset() {
                                log_ui_error(
                                    app,
                                    "Failed to delete special task preset",
                                    e.as_ref(),
                                );
                            }
                        }
                        KeyCode::Enter => {
                            app.apply_selected_special_task_preset();
                            app.input_mode = InputMode::ViewingSpecialTopics;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let len = app.all_task_filter_presets().len();
                            app.move_preset_down(len);
                        }
                        KeyCode::Up | KeyCode::Char('k') => app.move_preset_up(),
                        _ => {}
                    },
                    InputMode::SavingPreset | InputMode::SavingSpecialPreset => match key.code {
                        KeyCode::Esc => {
                            let special = matches!(app.input_mode, InputMode::SavingSpecialPreset);
                            app.clear_preset_form();
                            app.input_mode = if special {
                                InputMode::PresetSpecialFilters
                            } else {
                                InputMode::PresetFilters
                            };
                        }
                        KeyCode::Enter => {
                            let special = matches!(app.input_mode, InputMode::SavingSpecialPreset);
                            if let Err(e) = app.save_named_task_preset(special) {
                                app.preset_form_message = Some(e.to_string());
                                log_ui_error(app, "Failed to save task preset", e.as_ref());
                            } else {
                                app.clear_preset_form();
                                app.input_mode = if special {
                                    InputMode::PresetSpecialFilters
                                } else {
                                    InputMode::PresetFilters
                                };
                            }
                        }
                        KeyCode::Backspace => {
                            app.preset_form_message = None;
                            app.preset_name_input.pop();
                        }
                        KeyCode::Char(c) => {
                            app.preset_form_message = None;
                            app.preset_name_input.push(c);
                        }
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
                        KeyCode::Char(':') => {
                            app.begin_command_palette();
                        }
                        _ => {}
                    },
                    InputMode::ViewingSpecialTopics => match key.code {
                        KeyCode::Char(':') => {
                            app.begin_command_palette();
                        }
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
                Constraint::Length(5),  // Instructions
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
    let topic_title = format!("Topics [{}]", app.topics.len());
    let tabs = Tabs::new(titles)
        .select(if app.topics.is_empty() {
            0
        } else {
            app.selected_topic
        })
        .block(ui_style::surface_block(&topic_title, Accent::Tasks))
        .highlight_style(ui_style::title_style(Accent::Tasks))
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
                            ui_style::focused_inline_style(),
                        ),
                        highlighted_spans(
                            &summary,
                            &app.task_filter,
                            ui_style::info_style(),
                            ui_style::focused_inline_style(),
                        ),
                        task_status_spans(task),
                        Spans::from(Span::styled(
                            format!(
                                "ID {} | Created {} | Topic {}",
                                task.id, task.created_at, task.topic_id
                            ),
                            ui_style::muted_style(),
                        )),
                    ]
                } else {
                    vec![
                        highlighted_spans(
                            &task.name,
                            &app.task_filter,
                            title_style,
                            ui_style::focused_inline_style(),
                        ),
                        highlighted_spans(
                            &summary,
                            &app.task_filter,
                            ui_style::info_style(),
                            ui_style::focused_inline_style(),
                        ),
                        task_status_spans(task),
                    ]
                };
                ListItem::new(lines)
            })
            .collect()
    };

    let tasks_title = if app.has_task_filter() {
        format!(
            "Tasks [shown {} / total {}] | Filter: {}",
            filtered_indices.len(),
            app.tasks.len(),
            app.task_filter
        )
    } else {
        format!(
            "Tasks [shown {} / total {}]",
            filtered_indices.len(),
            app.tasks.len()
        )
    };
    let tasks_list = List::new(items)
        .block(ui_style::surface_block(&tasks_title, Accent::Tasks))
        .highlight_style(ui_style::selected_style())
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

    let command_lines =
        match app.input_mode {
            InputMode::Normal => vec![
                ui_style::command_bar_spans(&[
                    ("a", "add task"),
                    ("A", "add topic"),
                    ("e", "edit"),
                    ("d", "delete"),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "expand"),
                    ("Space", "done"),
                    ("/", "filter"),
                    ("p", "presets"),
                    (":", "palette"),
                ]),
                ui_style::command_bar_spans(&[
                    ("S", "special"),
                    ("f", "favorite"),
                    ("H", "help"),
                    ("q", "quit"),
                ]),
            ],
            InputMode::Filtering => vec![
                Spans::from(vec![
                    Span::raw("Query "),
                    Span::styled(
                        app.task_filter.clone(),
                        ui_style::title_style(Accent::Tasks),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "keep filter"),
                    ("Esc", "clear"),
                    ("status:", "done/open"),
                    ("topic:", "topic"),
                    ("fav:", "favorite"),
                ]),
            ],
            InputMode::CommandPalette => vec![
                Spans::from(vec![
                    Span::raw("Palette "),
                    Span::styled(
                        app.command_palette_query.clone(),
                        ui_style::title_style(Accent::Tasks),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "run command"),
                    ("j/k", "move"),
                    ("Backspace", "edit query"),
                    ("Esc", "close"),
                ]),
            ],
            InputMode::AddingTopic => vec![
                Spans::from(vec![
                    Span::raw("Topic "),
                    Span::styled(app.input.clone(), ui_style::title_style(Accent::Tasks)),
                ]),
                ui_style::command_bar_spans(&[("Enter", "create"), ("Esc", "cancel")]),
            ],
            InputMode::ViewingSpecialTopics => vec![
                ui_style::command_bar_spans(&[
                    ("Tab", "switch tab"),
                    ("/", "filter"),
                    ("p", "presets"),
                    ("d", "delete"),
                    ("f", "favorite"),
                ]),
                ui_style::command_bar_spans(&[(":", "palette"), ("Esc", "close"), ("H", "help")]),
            ],
            InputMode::FilteringSpecial => vec![
                Spans::from(vec![
                    Span::raw("Special "),
                    Span::styled(
                        app.special_task_filter.clone(),
                        ui_style::title_style(Accent::Tasks),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "keep filter"),
                    ("Esc", "clear"),
                    ("status:", "done/open"),
                    ("fav:", "favorite"),
                ]),
            ],
            InputMode::PresetFilters | InputMode::PresetSpecialFilters => {
                vec![ui_style::command_bar_spans(&[
                    ("Enter", "apply"),
                    ("S", "save current"),
                    ("x", "delete saved"),
                    ("Esc", "close"),
                ])]
            }
            InputMode::SavingPreset | InputMode::SavingSpecialPreset => vec![
                ui_style::command_bar_spans(&[("Enter", "save preset"), ("Esc", "cancel")]),
            ],
            InputMode::DeleteTask | InputMode::DeleteSpecialTask => vec![
                ui_style::command_bar_spans(&[("y", "confirm delete"), ("n", "cancel")]),
            ],
            InputMode::Help => vec![ui_style::command_bar_spans(&[("Esc", "close help")])],
            InputMode::AddingTaskName
            | InputMode::AddingTaskDescription
            | InputMode::EditingTaskName
            | InputMode::EditingTaskDescription => vec![ui_style::command_bar_spans(&[
                ("Tab", "switch field"),
                ("Enter", "save"),
                ("Esc", "cancel"),
            ])],
        };
    let help_message = Paragraph::new(command_lines)
        .style(ui_style::info_style())
        .block(ui_style::command_bar_block("Commands"));
    f.render_widget(help_message, chunks[2]);

    // (Optional) Show current mode at the bottom.
    let mode_text = match app.input_mode {
        InputMode::Normal => "Normal Mode",
        InputMode::CommandPalette => "Command Palette",
        InputMode::Filtering => "Filtering Tasks",
        InputMode::AddingTaskName => "Adding Task - Name Input",
        InputMode::AddingTaskDescription => "Adding Task - Description Input",
        InputMode::EditingTaskName => "Editing Task - Name Input",
        InputMode::EditingTaskDescription => "Editing Task - Description Input",
        InputMode::PresetFilters => "Task Presets",
        InputMode::PresetSpecialFilters => "Special Task Presets",
        InputMode::SavingPreset => "Saving Task Preset",
        InputMode::SavingSpecialPreset => "Saving Special Preset",
        InputMode::DeleteTask => "Delete Task",
        InputMode::DeleteSpecialTask => "Delete Task",
        InputMode::AddingTopic => "Adding Topic",
        InputMode::Help => "Viewing Help",
        InputMode::ViewingSpecialTopics => "Viewing Special Topics",
        InputMode::FilteringSpecial => "Filtering Special Tasks",
    };
    let mode = Paragraph::new(mode_text)
        .style(ui_style::body_style())
        .block(ui_style::shell_block("Mode"));
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
    let logs_list = List::new(visible_logs).block(ui_style::shell_block("Logs"));
    f.render_widget(logs_list, chunks[4]);

    // --- HELP POPUP (if enabled) ---
    if app.input_mode == InputMode::Help {
        let help_text = get_help_text();
        let help_paragraph = Paragraph::new(help_text)
            .block(ui_style::popup_block("Help", Accent::Tasks))
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .wrap(Wrap { trim: true });
        let area = ui_style::popup_rect(PopupSize::Tall, size);
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

    if matches!(
        app.input_mode,
        InputMode::SavingPreset | InputMode::SavingSpecialPreset
    ) {
        draw_save_task_preset_popup(f, app);
    }

    if app.input_mode == InputMode::CommandPalette {
        draw_command_palette_popup(f, app, size);
    }
}

fn draw_command_palette_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
    let popup_area = ui_style::popup_rect(PopupSize::Wide, size);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(popup_area);

    let input = Paragraph::new(app.command_palette_query.as_str())
        .style(ui_style::body_style())
        .block(ui_style::popup_block("Command Palette", Accent::Tasks));
    f.render_widget(input, layout[0]);
    f.set_cursor(
        layout[0].x + app.command_palette_query.len() as u16 + 1,
        layout[0].y + 1,
    );

    let commands = visible_task_palette_commands(app);
    let items: Vec<ListItem> = if commands.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No matching commands.",
            ui_style::muted_style(),
        ))])]
    } else {
        commands
            .iter()
            .map(|command| {
                ListItem::new(vec![
                    Spans::from(vec![
                        Span::styled(command.group, ui_style::muted_style()),
                        Span::raw("  "),
                        Span::styled(command.label, ui_style::title_style(Accent::Tasks)),
                        Span::raw("  "),
                        Span::styled(command.shortcut, ui_style::info_style()),
                    ]),
                    Spans::from(Span::styled(command.description, ui_style::muted_style())),
                ])
            })
            .collect()
    };

    let list = List::new(items)
        .block(ui_style::popup_block("Matches", Accent::Tasks))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");
    let mut state = ListState::default();
    if !commands.is_empty() {
        state.select(Some(app.command_palette_selected.min(commands.len() - 1)));
    }
    f.render_stateful_widget(list, layout[1], &mut state);

    let footer = Paragraph::new(vec![
        ui_style::command_bar_spans(&[("Enter", "run"), ("j/k", "move"), ("Esc", "close")]),
        Spans::from(Span::styled(
            "Recent commands rank first when the query is empty or ambiguous.",
            ui_style::muted_style(),
        )),
    ])
    .style(ui_style::info_style())
    .block(ui_style::popup_block("Palette Controls", Accent::Tasks));
    f.render_widget(footer, layout[2]);
}

fn draw_add_task_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Create a popup for task creation
    let size = f.size();
    let popup_area = ui_style::popup_rect(PopupSize::Standard, size);
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
    .style(ui_style::title_style(Accent::Tasks))
    .block(ui_style::popup_block("Task Editor", Accent::Tasks));
    f.render_widget(popup_title, popup_layout[0]);

    // Name field
    let name_input_style = if matches!(
        app.input_mode,
        InputMode::AddingTaskName | InputMode::EditingTaskName
    ) {
        ui_style::title_style(Accent::Tasks)
    } else {
        ui_style::muted_style()
    };

    let name_input = Paragraph::new(app.task_name_input.as_ref())
        .style(name_input_style)
        .block(ui_style::popup_block("Task Name", Accent::Tasks));
    f.render_widget(name_input, popup_layout[1]);

    // Description field
    let desc_input_style = if matches!(
        app.input_mode,
        InputMode::AddingTaskDescription | InputMode::EditingTaskDescription
    ) {
        ui_style::title_style(Accent::Tasks)
    } else {
        ui_style::muted_style()
    };

    let desc_input = Paragraph::new(app.task_description_input.as_ref())
        .style(desc_input_style)
        .block(ui_style::popup_block("Task Description", Accent::Tasks))
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
        ui_style::danger_style()
    } else {
        ui_style::subtle_style()
    };
    let feedback = Paragraph::new(feedback_text)
        .style(feedback_style)
        .block(ui_style::popup_block("Feedback", Accent::Tasks));
    f.render_widget(feedback, popup_layout[4]);

    let instructions_text = Paragraph::new(instructions)
        .style(ui_style::body_style())
        .block(ui_style::popup_block("Instructions", Accent::Tasks));
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
    let delete_popup_area = ui_style::popup_rect(PopupSize::Compact, size);
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
        .style(ui_style::title_style(Accent::Tasks))
        .alignment(tui::layout::Alignment::Center)
        .block(ui_style::popup_block("Delete Task", Accent::Tasks));
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
    .style(ui_style::danger_style())
    .alignment(tui::layout::Alignment::Center)
    .block(ui_style::popup_block("Confirmation", Accent::Tasks));
    f.render_widget(delete_message, popup_layout[1]);

    // Instructions
    let instructions = Paragraph::new("Press [Y] to confirm deletion or [N] to cancel")
        .style(ui_style::info_style())
        .alignment(tui::layout::Alignment::Center)
        .block(ui_style::popup_block("Controls", Accent::Tasks));
    f.render_widget(instructions, popup_layout[2]);
}

/// Build a single help line with a title, key command, and description.
fn build_help_line(
    title: &'static str,
    key: &'static str,
    description: &'static str,
) -> Spans<'static> {
    Spans::from(vec![
        Span::styled(title, ui_style::info_style().add_modifier(Modifier::BOLD)),
        Span::raw(" Press "),
        Span::styled(key, ui_style::title_style(Accent::Tasks)),
        Span::raw(" "),
        Span::styled(description, ui_style::body_style()),
    ])
}

/// Returns the help text as a vector of Spans.
pub fn get_help_text() -> Vec<Spans<'static>> {
    vec![
        Spans::from(Span::styled(
            "Help - Available Operations",
            ui_style::title_style(Accent::Tasks),
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
    let popup_area = ui_style::popup_rect(PopupSize::Full, size);
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
        .block(ui_style::popup_block("Special Tasks", Accent::Tasks))
        .highlight_style(ui_style::title_style(Accent::Tasks))
        .divider(Span::raw("|"));
    f.render_widget(tabs, popup_layout[0]);

    // Task list
    let tasks = app.get_current_special_tasks();
    let filtered_indices = app.filtered_special_task_indices();
    let items: Vec<ListItem> = if tasks.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No tasks found.",
            ui_style::muted_style().add_modifier(Modifier::ITALIC),
        ))])]
    } else if filtered_indices.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            format!("No tasks match \"{}\".", app.special_task_filter),
            ui_style::muted_style().add_modifier(Modifier::ITALIC),
        ))])]
    } else {
        filtered_indices
            .iter()
            .map(|task_index| {
                let task = &tasks[*task_index];
                let description_style = if task.completed {
                    ui_style::success_style()
                } else {
                    ui_style::info_style()
                };
                // If task is expanded, show extra details
                let lines = if app.expanded.contains(&task.id) {
                    vec![
                        highlighted_spans(
                            &task.name,
                            &app.special_task_filter,
                            description_style,
                            ui_style::focused_inline_style(),
                        ),
                        highlighted_spans(
                            &format!("Description: {}", task.description),
                            &app.special_task_filter,
                            description_style,
                            ui_style::focused_inline_style(),
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
                            ui_style::muted_style(),
                        )),
                    ]
                } else {
                    vec![highlighted_spans(
                        &format!("{}: {}", task.name, task.description),
                        &app.special_task_filter,
                        description_style,
                        ui_style::focused_inline_style(),
                    )]
                };
                ListItem::new(lines)
            })
            .collect()
    };

    let tasks_title = if app.has_special_task_filter() {
        format!(
            "Tasks [shown {} / total {}] | Filter: {}",
            filtered_indices.len(),
            tasks.len(),
            app.special_task_filter
        )
    } else {
        format!(
            "Tasks [shown {} / total {}]",
            filtered_indices.len(),
            tasks.len()
        )
    };
    let tasks_list = List::new(items)
        .block(ui_style::popup_block(&tasks_title, Accent::Tasks))
        .highlight_style(ui_style::selected_style())
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
        .style(ui_style::info_style())
        .block(ui_style::popup_block("Instructions", Accent::Tasks));
    f.render_widget(instructions_text, popup_layout[2]);

    // Show delete popup if needed
    if app.input_mode == InputMode::DeleteSpecialTask {
        let tasks = app.get_current_special_tasks();
        if let Some(task) = tasks.get(app.special_task_selected) {
            let delete_area = ui_style::popup_rect(PopupSize::Compact, size);
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
                .style(ui_style::title_style(Accent::Tasks))
                .alignment(tui::layout::Alignment::Center)
                .block(ui_style::popup_block("Delete Task", Accent::Tasks));
            f.render_widget(delete_title, delete_layout[0]);

            let delete_msg = Paragraph::new(format!(
                "Are you sure you want to delete \"{}\"?",
                task.name
            ))
            .style(ui_style::danger_style())
            .alignment(tui::layout::Alignment::Center)
            .block(ui_style::popup_block("Confirmation", Accent::Tasks));
            f.render_widget(delete_msg, delete_layout[1]);

            let delete_instructions = Paragraph::new("Press [Y] to confirm or [N] to cancel")
                .style(ui_style::info_style())
                .alignment(tui::layout::Alignment::Center)
                .block(ui_style::popup_block("Controls", Accent::Tasks));
            f.render_widget(delete_instructions, delete_layout[2]);
        }
    }

    if app.input_mode == InputMode::PresetSpecialFilters {
        draw_task_presets_popup(f, app, true);
    }
}

fn draw_task_presets_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, special: bool) {
    let size = f.size();
    let popup_area = ui_style::popup_rect(PopupSize::Standard, size);
    f.render_widget(Clear, popup_area);

    let presets = app.all_task_filter_presets();
    let items: Vec<ListItem> = presets
        .iter()
        .map(|(name, query, builtin)| {
            ListItem::new(vec![
                Spans::from(Span::styled(
                    name.clone(),
                    ui_style::title_style(Accent::Tasks),
                )),
                Spans::from(Span::styled(query.clone(), ui_style::muted_style())),
                Spans::from(Span::styled(
                    if *builtin {
                        "Built-in preset".to_string()
                    } else {
                        "Saved preset".to_string()
                    },
                    ui_style::subtle_style(),
                )),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(ui_style::popup_block(
            if special {
                "Special Task Presets (Enter apply, S save current, x delete saved)"
            } else {
                "Task Presets (Enter apply, S save current, x delete saved)"
            },
            Accent::Tasks,
        ))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    state.select(Some(app.preset_selected));
    f.render_stateful_widget(list, popup_area, &mut state);
}

fn draw_save_task_preset_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let popup_area = ui_style::popup_rect(PopupSize::Compact, size);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(popup_area);

    let title = Paragraph::new(
        if matches!(app.input_mode, InputMode::SavingSpecialPreset) {
            "Save Special Task Preset"
        } else {
            "Save Task Preset"
        },
    )
    .style(ui_style::title_style(Accent::Tasks))
    .block(ui_style::popup_block("Preset", Accent::Tasks));
    f.render_widget(title, layout[0]);

    let input = Paragraph::new(app.preset_name_input.as_str())
        .style(ui_style::body_style())
        .block(ui_style::popup_block("Preset Name", Accent::Tasks));
    f.render_widget(input, layout[1]);
    f.set_cursor(
        layout[1].x + app.preset_name_input.len() as u16 + 1,
        layout[1].y + 1,
    );

    let active_query = if matches!(app.input_mode, InputMode::SavingSpecialPreset) {
        app.special_task_filter.as_str()
    } else {
        app.task_filter.as_str()
    };
    let feedback = Paragraph::new(
        app.preset_form_message
            .clone()
            .unwrap_or_else(|| format!("Query: {active_query}")),
    )
    .style(if app.preset_form_message.is_some() {
        ui_style::danger_style()
    } else {
        ui_style::subtle_style()
    })
    .block(ui_style::popup_block("Feedback", Accent::Tasks));
    f.render_widget(feedback, layout[2]);
}
