use crate::common::command_palette::{visible_commands, PaletteCommand};
use crate::task_manager::app::{App, InputMode};
use crossterm::event::{KeyCode, KeyEvent};

pub enum UiAction {
    Continue,
    Exit,
}

fn log_ui_error(app: &mut App, context: &str, error: &dyn std::error::Error) {
    app.add_log("ERROR", &format!("{context}: {error}"));
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

pub(crate) fn visible_task_palette_commands(app: &App) -> Vec<PaletteCommand> {
    visible_commands(
        task_palette_commands(app),
        &app.command_palette_query,
        &app.recent_palette_commands,
    )
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

pub fn handle_key(app: &mut App, key: KeyEvent) -> Result<UiAction, Box<dyn std::error::Error>> {
    match app.input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => return Ok(UiAction::Exit),
            KeyCode::Char(':') => app.begin_command_palette(),
            KeyCode::Char('p') => app.begin_task_presets(),
            KeyCode::Char('/') => app.begin_task_filter(),
            KeyCode::Char('W') => {
                app.input_mode = InputMode::ViewingSpecialTopics;
                app.special_tab_selected = 0;
                if let Err(e) = app.load_special_tasks() {
                    app.input_mode = InputMode::Normal;
                    log_ui_error(app, "Failed to load special tasks", e.as_ref());
                }
            }
            KeyCode::Char('a') => app.begin_add_task(),
            KeyCode::Char('d') => app.begin_delete_task(),
            KeyCode::Char('e') => app.begin_edit_task(),
            KeyCode::Char('f') => {
                if let Err(e) = app.toggle_favourite() {
                    log_ui_error(app, "Failed to toggle favourite", e.as_ref());
                }
            }
            KeyCode::Char('H') => app.input_mode = InputMode::Help,
            KeyCode::Char('t') => {
                if let Err(e) = app.toggle_task() {
                    log_ui_error(app, "Failed to toggle task", e.as_ref());
                }
            }
            KeyCode::Enter => {
                if let Some(task) = app.tasks.get(app.selected) {
                    if app.expanded.contains(&task.id) {
                        app.expanded.remove(&task.id);
                    } else {
                        app.expanded.insert(task.id);
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => app.move_selection_down(),
            KeyCode::Up | KeyCode::Char('k') => app.move_selection_up(),
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
            KeyCode::PageUp => app.log_offset += 1,
            KeyCode::PageDown => {
                if app.log_offset > 0 {
                    app.log_offset -= 1;
                }
            }
            KeyCode::Char('N') => app.begin_add_topic(),
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
            KeyCode::Esc => app.close_command_palette(),
            KeyCode::Enter => {
                if let Some(command) = visible_task_palette_commands(app)
                    .get(app.command_palette_selected)
                    .copied()
                {
                    app.close_command_palette();
                    if let Err(e) = execute_task_palette_command(app, command.id) {
                        log_ui_error(app, "Failed to execute palette command", e.as_ref());
                    }
                } else {
                    app.close_command_palette();
                }
            }
            KeyCode::Backspace => {
                app.command_palette_query.pop();
                app.command_palette_selected = 0;
            }
            KeyCode::Up => {
                if app.command_palette_selected > 0 {
                    app.command_palette_selected -= 1;
                }
            }
            KeyCode::Down => {
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
            KeyCode::Enter => app.input_mode = InputMode::Normal,
            KeyCode::Backspace => app.pop_task_filter_char(),
            KeyCode::Char(c) => app.append_task_filter_char(c),
            _ => {}
        },
        InputMode::FilteringSpecial => match key.code {
            KeyCode::Esc => {
                app.clear_special_task_filter();
                app.input_mode = InputMode::ViewingSpecialTopics;
            }
            KeyCode::Enter => app.input_mode = InputMode::ViewingSpecialTopics,
            KeyCode::Backspace => app.pop_special_task_filter_char(),
            KeyCode::Char(c) => app.append_special_task_filter_char(c),
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
                    log_ui_error(app, "Failed to delete special task preset", e.as_ref());
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
        InputMode::AddingTaskDescription | InputMode::EditingTaskDescription => match key.code {
            KeyCode::Enter => {
                if !app.task_name_input.trim().is_empty() {
                    let name_clone = app.task_name_input.clone();
                    let desc_clone = app.task_description_input.clone();
                    let result = if app.input_mode == InputMode::AddingTaskDescription {
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
                app.input_mode = if app.input_mode == InputMode::AddingTaskDescription {
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
        },
        InputMode::DeleteTask => match key.code {
            KeyCode::Char('y') => {
                if let Err(e) = app.delete_task() {
                    log_ui_error(app, "Failed to delete task", e.as_ref());
                }
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => app.input_mode = InputMode::Normal,
            _ => {}
        },
        InputMode::DeleteSpecialTask => match key.code {
            KeyCode::Char('y') => {
                if let Err(e) = app.delete_special_task() {
                    log_ui_error(app, "Failed to delete task", e.as_ref());
                }
                app.input_mode = InputMode::ViewingSpecialTopics;
            }
            KeyCode::Char('n') | KeyCode::Esc => app.input_mode = InputMode::ViewingSpecialTopics,
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
            KeyCode::Esc => app.input_mode = InputMode::Normal,
            KeyCode::Char(c) => app.input.push(c),
            KeyCode::Backspace => {
                app.input.pop();
            }
            _ => {}
        },
        InputMode::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('H') => app.input_mode = InputMode::Normal,
            KeyCode::Char(':') => app.begin_command_palette(),
            _ => {}
        },
        InputMode::ViewingSpecialTopics => match key.code {
            KeyCode::Char(':') => app.begin_command_palette(),
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
            KeyCode::Up | KeyCode::Char('k') => app.move_special_selection_up(),
            KeyCode::Down | KeyCode::Char('j') => app.move_special_selection_down(),
            KeyCode::Enter => {
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
            KeyCode::Char('d') => app.begin_delete_special_task(),
            KeyCode::Char('p') => app.begin_special_task_presets(),
            KeyCode::Char('/') => app.begin_special_task_filter(),
            KeyCode::Esc => app.input_mode = InputMode::Normal,
            _ => {}
        },
    }

    Ok(UiAction::Continue)
}

#[cfg(test)]
mod tests {
    use super::handle_key;
    use crate::task_manager::app::{App, InputMode};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(format!("task_manager_cli_task_ui_{unique}.db"))
    }

    #[test]
    fn command_palette_treats_j_and_k_as_query_text() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("palette_jk");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;
        app.begin_command_palette();

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        )?;
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        )?;

        assert_eq!(app.input_mode, InputMode::CommandPalette);
        assert_eq!(app.command_palette_query, "jk");

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
