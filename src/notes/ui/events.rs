use crate::common::command_palette::{visible_commands, PaletteCommand};
use crate::notes::app::{App, InputMode, NotesView};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub enum UiAction {
    Continue,
    Exit,
}

fn log_ui_error(app: &mut App, context: &str, error: &dyn std::error::Error) {
    app.add_log("ERROR", &format!("{context}: {error}"));
}

fn notes_palette_commands(app: &App) -> Vec<PaletteCommand> {
    match app.active_view {
        NotesView::Files => vec![
            PaletteCommand {
                id: "switch_to_db",
                shortcut: "Tab",
                group: "Navigate",
                label: "Switch to Database Notes",
                description: "Move from file browser to stored DB notes.",
                keywords: "switch view database tab",
            },
            PaletteCommand {
                id: "search_files",
                shortcut: "/",
                group: "Discover",
                label: "Search Files",
                description: "Search the notes tree by path, title, or tag.",
                keywords: "search files path title tag slash",
            },
            PaletteCommand {
                id: "create_file",
                shortcut: "a",
                group: "Create",
                label: "Create File",
                description: "Create a new file in the current directory.",
                keywords: "new file create a",
            },
            PaletteCommand {
                id: "create_directory",
                shortcut: "N",
                group: "Create",
                label: "Create Directory",
                description: "Create a new folder in the current directory.",
                keywords: "new directory folder create",
            },
            PaletteCommand {
                id: "open_shortcuts",
                shortcut: "p",
                group: "Navigate",
                label: "Open Shortcuts",
                description: "Jump to pinned folders and saved searches.",
                keywords: "shortcuts pinned searches p",
            },
            PaletteCommand {
                id: "daily_note",
                shortcut: "D",
                group: "Create",
                label: "Open Daily Note",
                description: "Create or open today's daily note.",
                keywords: "daily note journal d",
            },
            PaletteCommand {
                id: "help",
                shortcut: "H",
                group: "General",
                label: "Open Help",
                description: "Show notes shortcuts and modes.",
                keywords: "help shortcuts docs",
            },
        ],
        NotesView::Database => vec![
            PaletteCommand {
                id: "switch_to_files",
                shortcut: "Tab",
                group: "Navigate",
                label: "Switch to File Browser",
                description: "Move from DB notes back to the file browser.",
                keywords: "switch view files tab",
            },
            PaletteCommand {
                id: "filter_notes",
                shortcut: "/",
                group: "Discover",
                label: "Filter Notes",
                description: "Filter DB notes by title or body.",
                keywords: "filter notes search title body slash",
            },
            PaletteCommand {
                id: "add_note",
                shortcut: "a",
                group: "Create",
                label: "Add Note",
                description: "Create a new DB-backed note.",
                keywords: "new note create a",
            },
            PaletteCommand {
                id: "edit_note",
                shortcut: "e",
                group: "Edit",
                label: "Edit Note",
                description: "Edit the selected DB note.",
                keywords: "edit selected note",
            },
            PaletteCommand {
                id: "delete_note",
                shortcut: "d",
                group: "Edit",
                label: "Delete Note",
                description: "Delete the selected DB note.",
                keywords: "delete remove note",
            },
            PaletteCommand {
                id: "open_presets",
                shortcut: "p",
                group: "Discover",
                label: "Open Note Presets",
                description: "Apply or manage saved DB note filters.",
                keywords: "presets saved filters p",
            },
            PaletteCommand {
                id: "help",
                shortcut: "H",
                group: "General",
                label: "Open Help",
                description: "Show notes shortcuts and modes.",
                keywords: "help shortcuts docs",
            },
        ],
    }
}

pub(crate) fn visible_notes_palette_commands(app: &App) -> Vec<PaletteCommand> {
    visible_commands(
        notes_palette_commands(app),
        &app.command_palette_query,
        &app.recent_palette_commands,
    )
}

fn execute_notes_palette_command(
    app: &mut App,
    command_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match command_id {
        "switch_to_db" => {
            if app.active_view != NotesView::Database {
                app.toggle_active_view();
            }
        }
        "switch_to_files" => {
            if app.active_view != NotesView::Files {
                app.toggle_active_view();
            }
        }
        "search_files" => app.begin_file_search(),
        "create_file" => app.begin_create_file(),
        "create_directory" => app.begin_create_directory(),
        "open_shortcuts" => app.begin_file_shortcuts(),
        "daily_note" => app.create_or_open_daily_note()?,
        "filter_notes" => app.begin_note_filter(),
        "add_note" => app.begin_add_note(),
        "edit_note" => app.begin_edit_note(),
        "delete_note" => app.begin_delete_note(),
        "open_presets" => app.begin_note_presets(),
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
            KeyCode::Tab => app.toggle_active_view(),
            KeyCode::Char('p') => {
                if app.active_view == NotesView::Files {
                    app.begin_file_shortcuts();
                } else {
                    app.begin_note_presets();
                }
            }
            KeyCode::Char('/') => {
                if app.active_view == NotesView::Files {
                    app.begin_file_search();
                } else {
                    app.begin_note_filter();
                }
            }
            KeyCode::Char('a') => {
                if app.active_view == NotesView::Files {
                    app.begin_create_file();
                } else {
                    app.begin_add_note();
                }
            }
            KeyCode::Char('D') => {
                if app.active_view == NotesView::Files {
                    if let Err(e) = app.create_or_open_daily_note() {
                        log_ui_error(app, "Failed to open daily note", e.as_ref());
                    }
                }
            }
            KeyCode::Char('d') => {
                if app.active_view == NotesView::Files {
                    app.begin_delete_selected_entry();
                } else {
                    app.begin_delete_note();
                }
            }
            KeyCode::Char('N') => {
                if app.active_view == NotesView::Files {
                    app.begin_create_directory();
                }
            }
            KeyCode::Char('R') => {
                if app.active_view == NotesView::Files {
                    app.begin_rename_selected_entry();
                }
            }
            KeyCode::Char('M') => {
                if app.active_view == NotesView::Files {
                    app.begin_move_selected_entry();
                }
            }
            KeyCode::Char('C') => {
                if app.active_view == NotesView::Files {
                    app.begin_copy_selected_entry();
                }
            }
            KeyCode::Char('e') => {
                if app.active_view == NotesView::Files {
                    if let Err(e) = app.edit_selected_file_in_editor() {
                        log_ui_error(app, "Failed to edit file", e.as_ref());
                    }
                } else {
                    app.begin_edit_note();
                }
            }
            KeyCode::Char('i') => {
                if app.active_view == NotesView::Files {
                    if let Err(e) = app.begin_inline_file_edit() {
                        log_ui_error(app, "Failed to start inline edit", e.as_ref());
                    }
                }
            }
            KeyCode::Char('m') => {
                if app.active_view == NotesView::Files {
                    if let Err(e) = app.toggle_pin_current_directory() {
                        log_ui_error(app, "Failed to toggle pinned directory", e.as_ref());
                    }
                }
            }
            KeyCode::Char('r') => {
                if app.active_view == NotesView::Files {
                    if let Err(e) = app.refresh_file_browser() {
                        log_ui_error(app, "Failed to refresh file browser", e.as_ref());
                    }
                }
            }
            KeyCode::Enter => {
                if app.active_view == NotesView::Files {
                    if let Err(e) = app.open_selected_file_entry() {
                        log_ui_error(app, "Failed to open file entry", e.as_ref());
                    }
                } else if !app.notes.is_empty() {
                    app.input_mode = InputMode::ViewingNote;
                }
            }
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                if app.active_view == NotesView::Files {
                    if app.has_file_search() {
                        app.clear_file_search();
                    } else if let Err(e) = app.move_to_parent_directory() {
                        log_ui_error(app, "Failed to move to parent directory", e.as_ref());
                    }
                }
            }
            KeyCode::Char('H') => app.input_mode = InputMode::Help,
            KeyCode::Down | KeyCode::Char('j') => {
                if app.active_view == NotesView::Files {
                    app.move_file_selection_down();
                } else {
                    app.move_selection_down();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.active_view == NotesView::Files {
                    app.move_file_selection_up();
                } else {
                    app.move_selection_up();
                }
            }
            KeyCode::PageUp => {
                if app.active_view == NotesView::Files {
                    app.scroll_preview_up(8);
                } else {
                    app.log_offset += 1;
                }
            }
            KeyCode::PageDown => {
                if app.active_view == NotesView::Files {
                    app.scroll_preview_down(8);
                } else if app.log_offset > 0 {
                    app.log_offset -= 1;
                }
            }
            _ => {}
        },
        InputMode::CommandPalette => match key.code {
            KeyCode::Esc => app.close_command_palette(),
            KeyCode::Enter => {
                if let Some(command) = visible_notes_palette_commands(app)
                    .get(app.command_palette_selected)
                    .copied()
                {
                    app.close_command_palette();
                    if let Err(e) = execute_notes_palette_command(app, command.id) {
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
                let visible = visible_notes_palette_commands(app);
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
        InputMode::SearchingFiles => match key.code {
            KeyCode::Esc => {
                app.clear_file_search();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Char('S') => {
                if let Err(e) = app.save_current_file_search() {
                    log_ui_error(app, "Failed to save file search", e.as_ref());
                }
            }
            KeyCode::Enter => app.input_mode = InputMode::Normal,
            KeyCode::Backspace => app.pop_file_search_char(),
            KeyCode::Down => app.move_file_selection_down(),
            KeyCode::Up => app.move_file_selection_up(),
            KeyCode::Char(c) => app.append_file_search_char(c),
            _ => {}
        },
        InputMode::FileShortcuts => match key.code {
            KeyCode::Esc => app.input_mode = InputMode::Normal,
            KeyCode::Enter => {
                if let Err(e) = app.apply_selected_file_shortcut() {
                    log_ui_error(app, "Failed to open shortcut", e.as_ref());
                } else {
                    app.input_mode = InputMode::Normal;
                }
            }
            KeyCode::Char('x') => {
                if let Err(e) = app.delete_selected_file_shortcut() {
                    log_ui_error(app, "Failed to delete shortcut", e.as_ref());
                }
            }
            KeyCode::Down | KeyCode::Char('j') => app.move_file_shortcut_down(),
            KeyCode::Up | KeyCode::Char('k') => app.move_file_shortcut_up(),
            _ => {}
        },
        InputMode::FileLinks => match key.code {
            KeyCode::Esc => app.input_mode = InputMode::ViewingFile,
            KeyCode::Enter => {
                if let Err(e) = app.open_selected_related_link() {
                    log_ui_error(app, "Failed to open related note", e.as_ref());
                }
            }
            KeyCode::Down | KeyCode::Char('j') => app.move_file_link_down(),
            KeyCode::Up | KeyCode::Char('k') => app.move_file_link_up(),
            _ => {}
        },
        InputMode::Filtering => match key.code {
            KeyCode::Esc => {
                app.clear_note_filter();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => app.input_mode = InputMode::Normal,
            KeyCode::Backspace => app.pop_note_filter_char(),
            KeyCode::Char(c) => app.append_note_filter_char(c),
            _ => {}
        },
        InputMode::PresetFilters => match key.code {
            KeyCode::Esc => app.input_mode = InputMode::Normal,
            KeyCode::Char('S') => app.begin_save_note_preset(),
            KeyCode::Char('x') => {
                if let Err(e) = app.delete_selected_note_preset() {
                    log_ui_error(app, "Failed to delete note preset", e.as_ref());
                }
            }
            KeyCode::Enter => {
                app.apply_selected_note_preset();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = app.all_note_filter_presets().len();
                app.move_preset_down(len);
            }
            KeyCode::Up | KeyCode::Char('k') => app.move_preset_up(),
            _ => {}
        },
        InputMode::SavingPreset => match key.code {
            KeyCode::Esc => {
                app.clear_preset_form();
                app.input_mode = InputMode::PresetFilters;
            }
            KeyCode::Enter => {
                if let Err(e) = app.save_named_note_preset() {
                    app.preset_form_message = Some(e.to_string());
                    log_ui_error(app, "Failed to save note preset", e.as_ref());
                } else {
                    app.clear_preset_form();
                    app.input_mode = InputMode::PresetFilters;
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
        InputMode::AddingNote | InputMode::EditingNote => match key.code {
            KeyCode::Enter => {
                if app.editing_title {
                    if app.title_input.trim().is_empty() {
                        app.set_note_form_message("Note title cannot be empty");
                    } else {
                        app.clear_note_form_message();
                        app.editing_title = false;
                    }
                } else if app.input_mode == InputMode::AddingNote {
                    let title = app.title_input.clone();
                    let content = app.content_input.clone();
                    if let Err(e) = app.add_note(&title, &content) {
                        app.set_note_form_message(e.to_string());
                        log_ui_error(app, "Failed to add note", e.as_ref());
                    } else {
                        app.reset_inputs();
                        app.input_mode = InputMode::Normal;
                    }
                } else if app.input_mode == InputMode::EditingNote {
                    if let Some(note) = app.notes.get(app.selected) {
                        let title = app.title_input.clone();
                        let content = app.content_input.clone();
                        if let Err(e) = app.update_note(note.id, &title, &content) {
                            app.set_note_form_message(e.to_string());
                            log_ui_error(app, "Failed to update note", e.as_ref());
                        } else {
                            app.reset_inputs();
                            app.input_mode = InputMode::Normal;
                        }
                    }
                }
            }
            KeyCode::Tab => {
                app.clear_note_form_message();
                app.editing_title = !app.editing_title;
            }
            KeyCode::Esc => app.cancel_note_edit(),
            KeyCode::Char(c) => {
                app.clear_note_form_message();
                if app.editing_title {
                    app.title_input.push(c);
                } else {
                    app.content_input.push(c);
                }
                app.mark_note_form_dirty();
            }
            KeyCode::Backspace => {
                app.clear_note_form_message();
                if app.editing_title {
                    app.title_input.pop();
                } else {
                    app.content_input.pop();
                }
                app.mark_note_form_dirty();
            }
            _ => {}
        },
        InputMode::ViewingNote => match key.code {
            KeyCode::Char(':') => app.begin_command_palette(),
            KeyCode::Esc | KeyCode::Enter => app.input_mode = InputMode::Normal,
            _ => {}
        },
        InputMode::ViewingFile => match key.code {
            KeyCode::Char(':') => app.begin_command_palette(),
            KeyCode::Tab => app.toggle_file_view_links_focus(),
            KeyCode::Char('l') => {
                if app.file_view_links_focus {
                    if let Err(e) = app.open_selected_related_link() {
                        log_ui_error(app, "Failed to open related note", e.as_ref());
                    }
                } else {
                    app.begin_file_links();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.file_view_links_focus {
                    app.move_file_link_down();
                } else {
                    app.scroll_viewed_file_down(1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.file_view_links_focus {
                    app.move_file_link_up();
                } else {
                    app.scroll_viewed_file_up(1);
                }
            }
            KeyCode::PageUp => app.scroll_viewed_file_up(12),
            KeyCode::PageDown => app.scroll_viewed_file_down(12),
            KeyCode::Enter => {
                if app.file_view_links_focus {
                    if let Err(e) = app.open_selected_related_link() {
                        log_ui_error(app, "Failed to open related note", e.as_ref());
                    }
                } else {
                    app.input_mode = InputMode::Normal;
                }
            }
            KeyCode::Char('i') => {
                if let Err(e) = app.begin_inline_file_edit() {
                    log_ui_error(app, "Failed to start inline edit", e.as_ref());
                }
            }
            KeyCode::Char('e') => {
                if let Err(e) = app.edit_viewed_file_in_editor() {
                    log_ui_error(app, "Failed to edit file", e.as_ref());
                }
            }
            KeyCode::Esc | KeyCode::Backspace => app.input_mode = InputMode::Normal,
            _ => {}
        },
        InputMode::EditingFile => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                if let Err(e) = app.save_inline_file_edit() {
                    app.file_edit_message = Some(e.to_string());
                    log_ui_error(app, "Failed to save inline edit", e.as_ref());
                }
            } else {
                match key.code {
                    KeyCode::Esc => app.cancel_inline_file_edit(),
                    KeyCode::Enter => app.insert_file_edit_newline(),
                    KeyCode::Tab => app.insert_file_edit_tab(),
                    KeyCode::Backspace => app.backspace_file_edit(),
                    KeyCode::Left => app.move_file_edit_left(),
                    KeyCode::Right => app.move_file_edit_right(),
                    KeyCode::Up => app.move_file_edit_up(),
                    KeyCode::Down => app.move_file_edit_down(),
                    KeyCode::PageUp => app.scroll_file_edit_up(),
                    KeyCode::PageDown => app.scroll_file_edit_down(),
                    KeyCode::Char(c) => app.insert_file_edit_char(c),
                    _ => {}
                }
            }
        }
        InputMode::CreatingFile
        | InputMode::CreatingDirectory
        | InputMode::RenamingFileEntry
        | InputMode::MovingFileEntry
        | InputMode::CopyingFileEntry => match key.code {
            KeyCode::Esc => app.cancel_file_creation(),
            KeyCode::Enter => {
                let result = match app.input_mode {
                    InputMode::CreatingFile => app.create_file(),
                    InputMode::CreatingDirectory => app.create_directory(),
                    InputMode::RenamingFileEntry => app.rename_selected_entry(),
                    InputMode::MovingFileEntry => app.move_selected_entry(),
                    InputMode::CopyingFileEntry => app.copy_selected_entry(),
                    _ => Ok(()),
                };
                if let Err(e) = result {
                    app.set_file_form_message(e.to_string());
                    log_ui_error(app, "Failed to apply file action", e.as_ref());
                }
            }
            KeyCode::Backspace => {
                app.clear_file_form_message();
                app.file_name_input.pop();
            }
            KeyCode::Down => {
                if app.input_mode == InputMode::CreatingFile {
                    app.move_file_template_down();
                }
            }
            KeyCode::Up => {
                if app.input_mode == InputMode::CreatingFile {
                    app.move_file_template_up();
                }
            }
            KeyCode::Char(c) => {
                app.clear_file_form_message();
                app.file_name_input.push(c);
            }
            _ => {}
        },
        InputMode::DeletingFileEntry => match key.code {
            KeyCode::Char('y') => {
                if let Err(e) = app.delete_selected_entry() {
                    log_ui_error(app, "Failed to delete file entry", e.as_ref());
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.pending_file_path = None;
                app.input_mode = InputMode::Normal;
            }
            _ => {}
        },
        InputMode::DeleteNote => match key.code {
            KeyCode::Char('y') => {
                if let Err(e) = app.delete_note() {
                    log_ui_error(app, "Failed to delete note", e.as_ref());
                }
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => app.input_mode = InputMode::Normal,
            _ => {}
        },
        InputMode::Help => match key.code {
            KeyCode::Char(':') => app.begin_command_palette(),
            KeyCode::Esc | KeyCode::Char('H') => app.input_mode = InputMode::Normal,
            _ => {}
        },
    }

    Ok(UiAction::Continue)
}

#[cfg(test)]
mod tests {
    use super::handle_key;
    use crate::notes::app::{App, InputMode};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
        std::env::temp_dir().join(format!("task_manager_cli_notes_ui_{unique}.db"))
    }

    fn temp_notes_root(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}_{}",
            prefix,
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(format!("task_manager_cli_notes_ui_files_{unique}"))
    }

    #[test]
    fn create_directory_mode_treats_j_and_k_as_text() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("dir_input_jk");
        let notes_root = temp_notes_root("dir_input_jk");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_create_directory();

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        )?;
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        )?;

        assert_eq!(app.input_mode, InputMode::CreatingDirectory);
        assert_eq!(app.file_name_input, "jk");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }
}
