use crate::notes::app::{App, InputMode, NotesView};
use crate::ui_style::{self, Accent, PopupSize};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::time::{Duration, Instant};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap},
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

fn visible_notes_palette_commands(app: &App) -> Vec<PaletteCommand> {
    let query = app.command_palette_query.trim().to_lowercase();
    let mut commands = notes_palette_commands(app)
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

fn markdown_source_spans(line: &str) -> Spans<'static> {
    let mut spans = Vec::new();
    let trimmed = line.trim_start();
    let indent_len = line.len().saturating_sub(trimmed.len());
    if indent_len > 0 {
        spans.push(Span::styled(
            line[..indent_len].to_string(),
            ui_style::subtle_style(),
        ));
    }

    if trimmed.starts_with('#') {
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        let prefix_len = hashes.min(trimmed.len());
        spans.push(Span::styled(
            trimmed[..prefix_len].to_string(),
            ui_style::title_style(Accent::Notes),
        ));
        if trimmed.len() > prefix_len {
            spans.push(Span::styled(
                trimmed[prefix_len..].to_string(),
                ui_style::body_style(),
            ));
        }
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        spans.push(Span::styled(
            trimmed[..2].to_string(),
            ui_style::info_style(),
        ));
        if trimmed.len() > 2 {
            spans.push(Span::styled(
                trimmed[2..].to_string(),
                ui_style::body_style(),
            ));
        }
    } else if trimmed.starts_with("```") {
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default().fg(Color::Magenta),
        ));
    } else if trimmed.starts_with('>') {
        spans.push(Span::styled(trimmed.to_string(), ui_style::success_style()));
    } else {
        spans.push(Span::styled(trimmed.to_string(), ui_style::body_style()));
    }

    Spans::from(spans)
}

fn slice_line_for_view(line: &str, start_col: usize, width: usize) -> String {
    line.chars().skip(start_col).take(width).collect()
}

fn format_reference_list(references: &[crate::notes::app::NoteReference]) -> String {
    if references.is_empty() {
        "none".to_string()
    } else {
        references
            .iter()
            .map(|reference| reference.label.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub fn run<B: Backend>(
    mut app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    terminal.clear()?;
    loop {
        terminal.draw(|f| {
            draw_ui(f, &mut app);
        })?;

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
                        KeyCode::Tab => {
                            app.toggle_active_view();
                        }
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
                                    log_ui_error(
                                        app,
                                        "Failed to toggle pinned directory",
                                        e.as_ref(),
                                    );
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
                                    log_ui_error(
                                        app,
                                        "Failed to move to parent directory",
                                        e.as_ref(),
                                    );
                                }
                            }
                        }
                        KeyCode::Char('H') => {
                            app.input_mode = InputMode::Help;
                        }
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
                        KeyCode::Esc => {
                            app.close_command_palette();
                        }
                        KeyCode::Enter => {
                            if let Some(command) = visible_notes_palette_commands(app)
                                .get(app.command_palette_selected)
                                .copied()
                            {
                                app.close_command_palette();
                                if let Err(e) = execute_notes_palette_command(app, command.id) {
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
                        KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Backspace => {
                            app.pop_file_search_char();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.move_file_selection_down();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.move_file_selection_up();
                        }
                        KeyCode::Char(c) => {
                            app.append_file_search_char(c);
                        }
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
                        KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Backspace => {
                            app.pop_note_filter_char();
                        }
                        KeyCode::Char(c) => {
                            app.append_note_filter_char(c);
                        }
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
                            } else {
                                if app.input_mode == InputMode::AddingNote {
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
                        }
                        KeyCode::Tab => {
                            app.clear_note_form_message();
                            app.editing_title = !app.editing_title;
                        }
                        KeyCode::Esc => {
                            app.cancel_note_edit();
                        }
                        KeyCode::Char(c) => {
                            app.clear_note_form_message();
                            if app.editing_title {
                                app.title_input.push(c);
                            } else {
                                app.content_input.push(c);
                            }
                        }
                        KeyCode::Backspace => {
                            app.clear_note_form_message();
                            if app.editing_title {
                                app.title_input.pop();
                            } else {
                                app.content_input.pop();
                            }
                        }
                        _ => {}
                    },
                    InputMode::ViewingNote => match key.code {
                        KeyCode::Char(':') => {
                            app.begin_command_palette();
                        }
                        KeyCode::Esc | KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                    InputMode::ViewingFile => match key.code {
                        KeyCode::Char(':') => {
                            app.begin_command_palette();
                        }
                        KeyCode::Tab => {
                            app.toggle_file_view_links_focus();
                        }
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
                        KeyCode::PageUp => {
                            app.scroll_viewed_file_up(12);
                        }
                        KeyCode::PageDown => {
                            app.scroll_viewed_file_down(12);
                        }
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
                        KeyCode::Esc | KeyCode::Backspace => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                    InputMode::EditingFile => {
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('s')
                        {
                            if let Err(e) = app.save_inline_file_edit() {
                                app.file_edit_message = Some(e.to_string());
                                log_ui_error(app, "Failed to save inline edit", e.as_ref());
                            }
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    app.cancel_inline_file_edit();
                                }
                                KeyCode::Enter => {
                                    app.insert_file_edit_newline();
                                }
                                KeyCode::Tab => {
                                    app.insert_file_edit_tab();
                                }
                                KeyCode::Backspace => {
                                    app.backspace_file_edit();
                                }
                                KeyCode::Left => {
                                    app.move_file_edit_left();
                                }
                                KeyCode::Right => {
                                    app.move_file_edit_right();
                                }
                                KeyCode::Up => {
                                    app.move_file_edit_up();
                                }
                                KeyCode::Down => {
                                    app.move_file_edit_down();
                                }
                                KeyCode::PageUp => {
                                    app.scroll_file_edit_up();
                                }
                                KeyCode::PageDown => {
                                    app.scroll_file_edit_down();
                                }
                                KeyCode::Char(c) => {
                                    app.insert_file_edit_char(c);
                                }
                                _ => {}
                            }
                        }
                    }
                    InputMode::CreatingFile
                    | InputMode::CreatingDirectory
                    | InputMode::RenamingFileEntry
                    | InputMode::MovingFileEntry
                    | InputMode::CopyingFileEntry => match key.code {
                        KeyCode::Esc => {
                            app.cancel_file_creation();
                        }
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
                        KeyCode::Down | KeyCode::Char('j') => {
                            if app.input_mode == InputMode::CreatingFile {
                                app.move_file_template_down();
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
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
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                    InputMode::Help => match key.code {
                        KeyCode::Char(':') => {
                            app.begin_command_palette();
                        }
                        KeyCode::Esc | KeyCode::Char('H') => {
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
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            [
                Constraint::Min(10),    // Notes list / Note view
                Constraint::Length(5),  // Instructions
                Constraint::Length(3),  // Mode indicator
                Constraint::Length(10), // Logs section
            ]
            .as_ref(),
        )
        .split(size);

    match app.input_mode {
        InputMode::ViewingNote => {
            draw_note_view(f, app, chunks[0]);
        }
        InputMode::ViewingFile => {
            draw_file_view(f, app, chunks[0]);
        }
        InputMode::EditingFile => {
            draw_inline_file_editor(f, app, chunks[0]);
        }
        InputMode::AddingNote | InputMode::EditingNote => {
            draw_edit_popup(f, app);
        }
        _ => {
            if app.active_view == NotesView::Files {
                draw_file_browser(f, app, chunks[0]);
            } else {
                draw_notes_list(f, app, chunks[0]);
            }
        }
    }

    let command_lines =
        match app.input_mode {
            InputMode::Normal => match app.active_view {
                NotesView::Files => vec![
                    ui_style::command_bar_spans(&[
                        ("Enter", "open"),
                        ("a", "new file"),
                        ("N", "new dir"),
                        ("R", "rename"),
                    ]),
                    ui_style::command_bar_spans(&[
                        ("M", "move"),
                        ("C", "copy"),
                        ("/", "search"),
                        (":", "palette"),
                        ("PgUp/PgDn", "preview"),
                    ]),
                    ui_style::command_bar_spans(&[
                        ("i", "inline edit"),
                        ("e", "external edit"),
                        ("p", "shortcuts"),
                        ("Tab", "switch view"),
                        ("H", "help"),
                    ]),
                ],
                NotesView::Database => vec![
                    ui_style::command_bar_spans(&[
                        ("a", "add note"),
                        ("e", "edit"),
                        ("d", "delete"),
                        ("Enter", "view"),
                    ]),
                    ui_style::command_bar_spans(&[
                        ("/", "filter"),
                        ("p", "presets"),
                        ("S", "save preset"),
                        (":", "palette"),
                    ]),
                    ui_style::command_bar_spans(&[
                        ("Tab", "switch view"),
                        ("H", "help"),
                        ("q", "quit"),
                    ]),
                ],
            },
            InputMode::SearchingFiles => vec![
                Spans::from(vec![
                    Span::raw("Search "),
                    Span::styled(
                        app.file_search_query.clone(),
                        ui_style::title_style(Accent::Notes),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "keep results"),
                    ("S", "save search"),
                    ("Esc", "clear"),
                    ("path:", "path"),
                    ("tag:", "tag"),
                ]),
            ],
            InputMode::CommandPalette => vec![
                Spans::from(vec![
                    Span::raw("Palette "),
                    Span::styled(
                        app.command_palette_query.clone(),
                        ui_style::title_style(Accent::Notes),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "run command"),
                    ("j/k", "move"),
                    ("Backspace", "edit query"),
                    ("Esc", "close"),
                ]),
            ],
            InputMode::Filtering => vec![
                Spans::from(vec![
                    Span::raw("Filter "),
                    Span::styled(
                        app.note_filter.clone(),
                        ui_style::title_style(Accent::Notes),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "keep filter"),
                    ("Esc", "clear"),
                    ("title:", "title"),
                    ("body:", "body"),
                    ("-", "negate"),
                ]),
            ],
            InputMode::ViewingFile => vec![ui_style::command_bar_spans(&[
                ("Tab", "focus links"),
                ("PgUp/PgDn", "scroll"),
                ("j/k", "scroll or move links"),
                ("Enter", "open selected link"),
                ("i", "inline edit"),
                ("e", "external edit"),
                ("l", "links popup"),
                ("Esc", "close"),
            ])],
            InputMode::EditingFile => vec![ui_style::command_bar_spans(&[
                ("Arrows", "move"),
                ("PgUp/PgDn", "scroll"),
                ("Ctrl+S", "save"),
                ("Esc", "cancel"),
            ])],
            InputMode::AddingNote | InputMode::EditingNote => vec![ui_style::command_bar_spans(&[
                ("Tab", "switch field"),
                ("Enter", "save"),
                ("Esc", "cancel"),
            ])],
            InputMode::CreatingFile => vec![ui_style::command_bar_spans(&[
                ("Up/Down", "template"),
                ("Enter", "create file"),
                ("Esc", "cancel"),
            ])],
            InputMode::CreatingDirectory
            | InputMode::RenamingFileEntry
            | InputMode::MovingFileEntry
            | InputMode::CopyingFileEntry => vec![ui_style::command_bar_spans(&[
                ("Enter", "confirm"),
                ("Esc", "cancel"),
            ])],
            InputMode::FileShortcuts => vec![ui_style::command_bar_spans(&[
                ("Enter", "open"),
                ("x", "remove"),
                ("Esc", "close"),
            ])],
            InputMode::FileLinks => vec![ui_style::command_bar_spans(&[
                ("Enter", "open note"),
                ("Up/Down", "move"),
                ("Esc", "close"),
            ])],
            InputMode::PresetFilters => vec![ui_style::command_bar_spans(&[
                ("Enter", "apply"),
                ("S", "save current"),
                ("x", "delete saved"),
                ("Esc", "close"),
            ])],
            InputMode::SavingPreset => vec![ui_style::command_bar_spans(&[
                ("Enter", "save preset"),
                ("Esc", "cancel"),
            ])],
            InputMode::DeletingFileEntry | InputMode::DeleteNote => vec![
                ui_style::command_bar_spans(&[("y", "confirm delete"), ("n", "cancel")]),
            ],
            InputMode::ViewingNote => vec![ui_style::command_bar_spans(&[
                ("Esc", "close"),
                ("Enter", "close"),
            ])],
            InputMode::Help => vec![ui_style::command_bar_spans(&[("Esc", "close help")])],
        };
    let help_message = Paragraph::new(command_lines)
        .style(ui_style::info_style())
        .block(ui_style::command_bar_block("Commands"));
    f.render_widget(help_message, chunks[1]);

    // Mode indicator
    let mode_text = match app.input_mode {
        InputMode::Normal => match app.active_view {
            NotesView::Files => "File Browser",
            NotesView::Database => "Notes List",
        },
        InputMode::CommandPalette => "Command Palette",
        InputMode::SearchingFiles => "Searching Files",
        InputMode::FileShortcuts => "File Shortcuts",
        InputMode::FileLinks => "Related Notes",
        InputMode::Filtering => "Filtering Notes",
        InputMode::PresetFilters => "Note Presets",
        InputMode::SavingPreset => "Saving Note Preset",
        InputMode::AddingNote => "Adding Note",
        InputMode::EditingNote => "Editing Note",
        InputMode::ViewingNote => "Viewing Note",
        InputMode::ViewingFile => "Viewing File",
        InputMode::EditingFile => "Editing File",
        InputMode::CreatingFile => "Creating File",
        InputMode::CreatingDirectory => "Creating Directory",
        InputMode::RenamingFileEntry => "Renaming File Entry",
        InputMode::MovingFileEntry => "Moving File Entry",
        InputMode::CopyingFileEntry => "Copying File Entry",
        InputMode::DeletingFileEntry => "Deleting File Entry",
        InputMode::DeleteNote => "Delete Note",
        InputMode::Help => "Viewing Help",
    };
    let mode = Paragraph::new(mode_text)
        .style(ui_style::body_style())
        .block(ui_style::shell_block("Mode"));
    f.render_widget(mode, chunks[2]);

    // Logs section
    let log_area_height = chunks[3].height as usize;
    let total_logs = app.logs.len();
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
    f.render_widget(logs_list, chunks[3]);

    // Help popup
    if app.input_mode == InputMode::Help {
        draw_help_popup(f, size);
    }

    // Delete confirmation popup
    if app.input_mode == InputMode::DeleteNote {
        draw_delete_popup(f, app, size);
    }

    if app.input_mode == InputMode::PresetFilters {
        draw_note_presets_popup(f, app, size);
    }

    if app.input_mode == InputMode::FileShortcuts {
        draw_file_shortcuts_popup(f, app, size);
    }

    if app.input_mode == InputMode::FileLinks {
        draw_file_links_popup(f, app, size);
    }

    if app.input_mode == InputMode::SavingPreset {
        draw_save_preset_popup(f, app, size);
    }

    if app.input_mode == InputMode::CommandPalette {
        draw_command_palette_popup(f, app, size);
    }

    if matches!(
        app.input_mode,
        InputMode::CreatingFile
            | InputMode::CreatingDirectory
            | InputMode::RenamingFileEntry
            | InputMode::MovingFileEntry
            | InputMode::CopyingFileEntry
    ) {
        draw_create_file_popup(f, app, size);
    }

    if app.input_mode == InputMode::DeletingFileEntry {
        draw_delete_file_popup(f, app, size);
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
        .block(ui_style::popup_block("Command Palette", Accent::Notes));
    f.render_widget(input, layout[0]);
    f.set_cursor(
        layout[0].x + app.command_palette_query.len() as u16 + 1,
        layout[0].y + 1,
    );

    let commands = visible_notes_palette_commands(app);
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
                        Span::styled(command.label, ui_style::title_style(Accent::Notes)),
                        Span::raw("  "),
                        Span::styled(command.shortcut, ui_style::info_style()),
                    ]),
                    Spans::from(Span::styled(command.description, ui_style::muted_style())),
                ])
            })
            .collect()
    };

    let list = List::new(items)
        .block(ui_style::popup_block("Matches", Accent::Notes))
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
    .block(ui_style::popup_block("Palette Controls", Accent::Notes));
    f.render_widget(footer, layout[2]);
}

fn draw_notes_list<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let filtered_indices = app.filtered_note_indices();
    let items: Vec<ListItem> = if app.notes.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No notes. Press 'a' to add a note.",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))])]
    } else if filtered_indices.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            format!("No notes match \"{}\".", app.note_filter),
            ui_style::muted_style().add_modifier(Modifier::ITALIC),
        ))])]
    } else {
        filtered_indices
            .iter()
            .map(|note_index| {
                let note = &app.notes[*note_index];
                let preview = if note.content.len() > 50 {
                    format!("{}...", &note.content[..50])
                } else {
                    note.content.clone()
                };
                ListItem::new(vec![
                    highlighted_spans(
                        &note.title,
                        &app.note_filter,
                        ui_style::title_style(Accent::Notes),
                        ui_style::focused_inline_style(),
                    ),
                    highlighted_spans(
                        &preview,
                        &app.note_filter,
                        ui_style::muted_style(),
                        ui_style::focused_inline_style(),
                    ),
                    Spans::from(Span::styled(
                        format!(
                            "Created: {} | Updated: {}",
                            note.created_at, note.updated_at
                        ),
                        ui_style::subtle_style(),
                    )),
                ])
            })
            .collect()
    };

    let notes_title = if app.has_note_filter() {
        format!("Notes | Filter: {}", app.note_filter)
    } else {
        "Notes".to_string()
    };
    let notes_list = List::new(items)
        .block(ui_style::surface_block(&notes_title, Accent::Notes))
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
    f.render_stateful_widget(notes_list, area, &mut list_state);
}

fn draw_note_view<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    if let Some(note) = app.notes.get(app.selected) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(area);

        let title = Paragraph::new(note.title.clone())
            .style(ui_style::title_style(Accent::Notes))
            .block(ui_style::surface_block("Title", Accent::Notes));
        f.render_widget(title, chunks[0]);

        let content = Paragraph::new(note.content.clone())
            .style(ui_style::body_style())
            .block(ui_style::surface_block("Content", Accent::Notes))
            .wrap(Wrap { trim: true });
        f.render_widget(content, chunks[1]);
    }
}

fn draw_file_browser<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    let visible_entries = app.visible_file_entries();
    let items: Vec<ListItem> = if visible_entries.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No files here. Press 'a' to create one.",
            ui_style::muted_style().add_modifier(Modifier::ITALIC),
        ))])]
    } else {
        visible_entries
            .iter()
            .map(|entry| {
                let style = if entry.is_dir {
                    ui_style::title_style(Accent::Notes)
                } else {
                    ui_style::body_style()
                };
                ListItem::new(vec![
                    Spans::from(vec![
                        ui_style::badge(if entry.is_dir { "DIR" } else { "FILE" }, Accent::Notes),
                        Span::raw(" "),
                        Span::styled(entry.name.clone(), style),
                    ]),
                    Spans::from(Span::styled(
                        format!(
                            "{} | {}",
                            entry
                                .modified_at
                                .clone()
                                .unwrap_or_else(|| "unknown time".to_string()),
                            if entry.is_dir {
                                "dir".to_string()
                            } else {
                                crate::notes::app::format_file_size(entry.size_bytes)
                            }
                        ),
                        ui_style::subtle_style(),
                    )),
                ])
            })
            .collect()
    };

    let files_title = if app.has_file_search() {
        format!(
            "Files | Find: {} | {}/{}",
            app.file_search_query,
            app.file_selected
                .saturating_add(1)
                .min(visible_entries.len()),
            visible_entries.len()
        )
    } else {
        format!(
            "Files {} | {}/{}",
            app.relative_current_dir(),
            app.file_selected
                .saturating_add(1)
                .min(visible_entries.len()),
            visible_entries.len()
        )
    };
    let list = List::new(items)
        .block(ui_style::surface_block(&files_title, Accent::Notes))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut list_state = ListState::default();
    if !visible_entries.is_empty() {
        list_state.select(Some(app.file_selected));
    }
    f.render_stateful_widget(list, panes[0], &mut list_state);

    let preview_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(panes[1]);

    let breadcrumb = Paragraph::new(format!(
        "Path: {} | {}",
        app.selected_file_breadcrumb(),
        app.preview_summary()
    ))
    .style(ui_style::info_style())
    .block(ui_style::surface_block("Preview Meta", Accent::Notes));
    f.render_widget(breadcrumb, preview_chunks[0]);

    let preview_title = app
        .previewed_file_path
        .as_ref()
        .map(|path| {
            format!(
                "Preview {} | lines {}-{} / {}",
                app.relative_path_from_root(path),
                app.preview_scroll.saturating_add(1),
                (app.preview_scroll + preview_chunks[1].height.saturating_sub(2) as usize)
                    .min(app.preview_line_count()),
                app.preview_line_count()
            )
        })
        .unwrap_or_else(|| "Preview".to_string());
    let preview_body = if let Some(path) = &app.previewed_file_path {
        if path.is_dir() {
            app.previewed_file_content.clone()
        } else {
            let refs = app.file_references(path);
            let backlinks = app.file_backlinks(path);
            format!(
                "{}\n\nReferences: {}\nBacklinks: {}",
                app.previewed_file_content,
                format_reference_list(&refs),
                format_reference_list(&backlinks)
            )
        }
    } else {
        app.previewed_file_content.clone()
    };
    let preview = Paragraph::new(preview_body)
        .style(ui_style::body_style())
        .block(ui_style::surface_block(&preview_title, Accent::Notes))
        .scroll((app.preview_scroll as u16, 0))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, preview_chunks[1]);

    let preview_status = Paragraph::new(
        if app.preview_line_count() > preview_chunks[1].height.saturating_sub(2) as usize {
            format!(
                "PgUp/PgDn scroll preview | offset {} of {}",
                app.preview_scroll,
                app.preview_line_count().saturating_sub(1)
            )
        } else {
            "Preview fits in view".to_string()
        },
    )
    .style(ui_style::muted_style())
    .block(ui_style::surface_block("Preview Status", Accent::Notes));
    f.render_widget(preview_status, preview_chunks[2]);
}

fn draw_file_view<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let metadata = app.viewed_file_metadata();
    let related_links = app.related_file_links();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
            Constraint::Length(7),
        ])
        .split(area);

    let path_text = app
        .viewed_file_path
        .as_ref()
        .map(|path| app.relative_path_from_root(path))
        .unwrap_or_else(|| "No file selected".to_string());

    let title = Paragraph::new(path_text)
        .style(ui_style::title_style(Accent::Notes))
        .block(ui_style::surface_block("File", Accent::Notes));
    f.render_widget(title, chunks[0]);

    let title_text = metadata
        .title
        .as_deref()
        .map(|title| format!("Title: {title}"))
        .unwrap_or_else(|| "Title: none".to_string());
    let tags_text = if metadata.tags.is_empty() {
        "Tags: none".to_string()
    } else {
        format!("Tags: {}", metadata.tags.join(", "))
    };
    let metadata_panel = Paragraph::new(format!("{title_text} | {tags_text}"))
        .style(ui_style::info_style())
        .block(ui_style::surface_block("Frontmatter", Accent::Notes))
        .wrap(Wrap { trim: false });
    f.render_widget(metadata_panel, chunks[1]);

    let content_title = format!(
        "Content | lines {}-{} / {}",
        app.viewed_file_scroll.saturating_add(1),
        (app.viewed_file_scroll + chunks[2].height.saturating_sub(2) as usize)
            .min(app.viewed_file_line_count()),
        app.viewed_file_line_count()
    );
    let content = Paragraph::new(app.viewed_file_content.as_str())
        .style(ui_style::body_style())
        .block(ui_style::surface_block(&content_title, Accent::Notes))
        .scroll((app.viewed_file_scroll as u16, 0))
        .wrap(Wrap { trim: false });
    f.render_widget(content, chunks[2]);

    let scroll_status = Paragraph::new(
        if app.viewed_file_line_count() > chunks[2].height.saturating_sub(2) as usize {
            if app.file_view_links_focus {
                "Links focused | j/k move links | PgUp/PgDn scroll content".to_string()
            } else {
                "Content focused | j/k or PgUp/PgDn scroll | Tab moves to links".to_string()
            }
        } else if app.file_view_links_focus {
            "Links focused | Enter opens selected link".to_string()
        } else {
            "Content fits in view | Tab moves to links".to_string()
        },
    )
    .style(ui_style::muted_style())
    .block(ui_style::surface_block("Scroll Status", Accent::Notes));
    f.render_widget(scroll_status, chunks[3]);

    let related_items: Vec<ListItem> = if related_links.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No references or backlinks.",
            ui_style::muted_style(),
        ))])]
    } else {
        related_links
            .iter()
            .map(|link| {
                ListItem::new(vec![Spans::from(vec![
                    if link.group.starts_with("Reference") {
                        ui_style::badge("REF", Accent::Notes)
                    } else {
                        ui_style::badge("BACK", Accent::Notes)
                    },
                    Span::raw(" "),
                    Span::styled(
                        link.label.clone(),
                        ui_style::body_style().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" -> {}", app.relative_path_from_root(&link.path)),
                        ui_style::subtle_style(),
                    ),
                ])])
            })
            .collect()
    };
    let related_list = List::new(related_items)
        .block(ui_style::surface_block(
            if app.file_view_links_focus {
                "Links (focused: Up/Down move, Enter open, Tab leave)"
            } else {
                "Links (Tab focus, l popup)"
            },
            Accent::Notes,
        ))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");
    let mut link_state = ListState::default();
    if !related_links.is_empty() {
        link_state.select(Some(app.file_link_selected));
    }
    f.render_stateful_widget(related_list, chunks[4], &mut link_state);
}

fn draw_edit_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let popup_area = ui_style::popup_rect(PopupSize::Full, size);
    f.render_widget(Clear, popup_area);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Title input
            Constraint::Min(10),   // Content input
            Constraint::Length(3), // Feedback
            Constraint::Length(3), // Instructions
        ])
        .split(popup_area);

    let popup_title = Paragraph::new(if app.input_mode == InputMode::AddingNote {
        "Create New Note"
    } else {
        "Edit Note"
    })
    .style(ui_style::title_style(Accent::Notes))
    .block(ui_style::popup_block("Note Editor", Accent::Notes));
    f.render_widget(popup_title, popup_layout[0]);

    let title_style = if app.editing_title {
        ui_style::title_style(Accent::Notes)
    } else {
        ui_style::info_style()
    };
    let title_input = Paragraph::new(app.title_input.as_ref())
        .style(title_style)
        .block(ui_style::popup_block("Title", Accent::Notes));
    f.render_widget(title_input, popup_layout[1]);

    let content_style = if app.editing_title {
        ui_style::muted_style()
    } else {
        ui_style::body_style()
    };
    let content_input = Paragraph::new(app.content_input.as_ref())
        .style(content_style)
        .block(ui_style::popup_block("Content", Accent::Notes))
        .wrap(Wrap { trim: true });
    f.render_widget(content_input, popup_layout[2]);

    let feedback_text = app
        .note_form_message
        .clone()
        .unwrap_or_else(|| "Enter a title, then content, then save.".to_string());
    let feedback_style = if app.note_form_message.is_some() {
        ui_style::danger_style()
    } else {
        ui_style::subtle_style()
    };
    let feedback = Paragraph::new(feedback_text)
        .style(feedback_style)
        .block(ui_style::popup_block("Feedback", Accent::Notes));
    f.render_widget(feedback, popup_layout[3]);

    // Set cursor position
    if app.editing_title {
        f.set_cursor(
            popup_layout[1].x + app.title_input.len() as u16 + 1,
            popup_layout[1].y + 1,
        );
    } else {
        // For content, cursor position is more complex with wrapping, so we'll just show it at the end
        let content_lines: Vec<&str> = app.content_input.lines().collect();
        let last_line = content_lines.last().unwrap_or(&"");
        f.set_cursor(
            popup_layout[2].x + last_line.len() as u16 + 1,
            popup_layout[2].y + content_lines.len() as u16,
        );
    }

    let instructions =
        Paragraph::new("Enter title and content. Press Enter to save, Esc to cancel.")
            .style(ui_style::info_style())
            .block(ui_style::popup_block("Instructions", Accent::Notes));
    f.render_widget(instructions, popup_layout[4]);
}

fn draw_delete_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
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
        .style(ui_style::title_style(Accent::Notes))
        .alignment(tui::layout::Alignment::Center)
        .block(ui_style::popup_block("Delete Note", Accent::Notes));
    f.render_widget(delete_title, delete_layout[0]);

    let note_name = if let Some(note) = app.notes.get(app.selected) {
        &note.title
    } else {
        "Unknown Note"
    };

    let delete_msg = Paragraph::new(format!(
        "Are you sure you want to delete \"{}\"?",
        note_name
    ))
    .style(ui_style::danger_style())
    .alignment(tui::layout::Alignment::Center)
    .block(ui_style::popup_block("Confirmation", Accent::Notes));
    f.render_widget(delete_msg, delete_layout[1]);

    let delete_instructions = Paragraph::new("Press [Y] to confirm or [N] to cancel")
        .style(ui_style::info_style())
        .alignment(tui::layout::Alignment::Center)
        .block(ui_style::popup_block("Controls", Accent::Notes));
    f.render_widget(delete_instructions, delete_layout[2]);
}

fn draw_help_popup<B: Backend>(f: &mut Frame<B>, size: Rect) {
    let help_area = ui_style::popup_rect(PopupSize::Tall, size);
    f.render_widget(Clear, help_area);

    let help_text = vec![
        Spans::from(Span::styled(
            "Notes App - Help",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from(""),
        Spans::from(Span::styled(
            "File Browser:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from("  Tab - Switch between file browser and database notes"),
        Spans::from("  Up/Down or j/k - Navigate entries"),
        Spans::from("  Enter - Open file or directory"),
        Spans::from("  / - Search files (supports path:, name:, title:, tag:, quotes, -negation)"),
        Spans::from("  Backspace/h - Clear search or move to parent directory"),
        Spans::from("  a - Create a file in the current directory"),
        Spans::from("  D - Create or open today's daily note"),
        Spans::from("  N - Create a directory"),
        Spans::from("  R - Rename selected file or directory"),
        Spans::from("  M - Move selected file or directory"),
        Spans::from("  C - Copy selected file or directory"),
        Spans::from("  d - Delete selected file or directory"),
        Spans::from("  i - Edit selected file inline"),
        Spans::from("  e - Edit selected file in $NOTES_EDITOR or $EDITOR"),
        Spans::from("  l - Open references/backlinks popup for the current file"),
        Spans::from("  Tab in file view - Focus direct links panel"),
        Spans::from("  m - Pin/unpin the current directory"),
        Spans::from("  p - Open pinned directories and saved searches"),
        Spans::from("  r - Refresh current directory"),
        Spans::from(""),
        Spans::from(Span::styled(
            "Database Notes:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from("  a - Add new note"),
        Spans::from("  e - Edit selected note"),
        Spans::from("  d - Delete selected note"),
        Spans::from("  / - Filter notes (supports title:, body:, quotes, and -negation)"),
        Spans::from("  p - Open preset note filters"),
        Spans::from("  S - Save the current filter as a preset"),
        Spans::from("  x - Delete a selected saved preset"),
        Spans::from("  H - Toggle help"),
        Spans::from("  q - Quit"),
        Spans::from(""),
        Spans::from(Span::styled(
            "Editing:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from("  Enter - Save note"),
        Spans::from("  Esc - Cancel"),
        Spans::from("  Ctrl+S - Save inline file edit"),
        Spans::from("  Arrows - Move cursor in inline file edit"),
    ];

    let help_paragraph = Paragraph::new(help_text)
        .block(ui_style::popup_block("Help", Accent::Notes))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .wrap(Wrap { trim: true });
    f.render_widget(help_paragraph, help_area);
}

fn draw_note_presets_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
    let popup_area = ui_style::popup_rect(PopupSize::Standard, size);
    f.render_widget(Clear, popup_area);

    let presets = app.all_note_filter_presets();
    let items: Vec<ListItem> = presets
        .iter()
        .map(|(name, query, builtin)| {
            ListItem::new(vec![
                Spans::from(Span::styled(
                    name.clone(),
                    ui_style::title_style(Accent::Notes),
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
            "Note Presets (Enter apply, S save current, x delete saved)",
            Accent::Notes,
        ))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    state.select(Some(app.preset_selected));
    f.render_stateful_widget(list, popup_area, &mut state);
}

fn draw_save_preset_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
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

    let title = Paragraph::new("Save Note Filter Preset")
        .style(ui_style::title_style(Accent::Notes))
        .block(ui_style::popup_block("Preset", Accent::Notes));
    f.render_widget(title, layout[0]);

    let input = Paragraph::new(app.preset_name_input.as_str())
        .style(ui_style::body_style())
        .block(ui_style::popup_block("Preset Name", Accent::Notes));
    f.render_widget(input, layout[1]);
    f.set_cursor(
        layout[1].x + app.preset_name_input.len() as u16 + 1,
        layout[1].y + 1,
    );

    let feedback = Paragraph::new(
        app.preset_form_message
            .clone()
            .unwrap_or_else(|| format!("Query: {}", app.note_filter)),
    )
    .style(if app.preset_form_message.is_some() {
        ui_style::danger_style()
    } else {
        ui_style::subtle_style()
    })
    .block(ui_style::popup_block("Feedback", Accent::Notes));
    f.render_widget(feedback, layout[2]);
}

fn draw_create_file_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
    let popup_area = ui_style::popup_rect(PopupSize::Standard, size);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Length(4),
            Constraint::Length(4),
        ])
        .split(popup_area);

    let title = Paragraph::new(match app.input_mode {
        InputMode::CreatingDirectory => "Create Directory",
        InputMode::RenamingFileEntry => "Rename Entry",
        InputMode::MovingFileEntry => "Move Entry",
        InputMode::CopyingFileEntry => "Copy Entry",
        _ => "Create File",
    })
    .style(ui_style::title_style(Accent::Notes))
    .block(ui_style::popup_block("File Action", Accent::Notes));
    f.render_widget(title, layout[0]);

    let path = Paragraph::new(format!("Folder: {}", app.relative_current_dir()))
        .style(ui_style::info_style())
        .block(ui_style::popup_block("Location", Accent::Notes));
    f.render_widget(path, layout[1]);

    let input = Paragraph::new(app.file_name_input.as_str())
        .style(ui_style::body_style())
        .block(ui_style::popup_block(
            match app.input_mode {
                InputMode::CreatingDirectory => "Directory Name",
                InputMode::RenamingFileEntry => "New Name",
                InputMode::MovingFileEntry | InputMode::CopyingFileEntry => {
                    "Destination Path (relative to notes root)"
                }
                _ => "File Name (.md added if omitted)",
            },
            Accent::Notes,
        ));
    f.render_widget(input, layout[2]);
    f.set_cursor(
        layout[2].x + app.file_name_input.len() as u16 + 1,
        layout[2].y + 1,
    );

    let template_body = if app.input_mode == InputMode::CreatingFile {
        app.all_file_templates()
            .iter()
            .enumerate()
            .map(|(index, template)| {
                let prefix = if index == app.file_template_selected {
                    "=> "
                } else {
                    "   "
                };
                Spans::from(Span::styled(
                    format!("{prefix}{}", template.name),
                    if index == app.file_template_selected {
                        ui_style::title_style(Accent::Notes)
                    } else {
                        ui_style::muted_style()
                    },
                ))
            })
            .collect::<Vec<_>>()
    } else {
        vec![Spans::from(Span::styled(
            "Template selection is only used for file creation.",
            ui_style::subtle_style(),
        ))]
    };
    let template_panel = Paragraph::new(template_body)
        .block(ui_style::popup_block("Template", Accent::Notes))
        .wrap(Wrap { trim: false });
    f.render_widget(template_panel, layout[3]);

    let feedback =
        Paragraph::new(
            app.file_form_message
                .clone()
                .unwrap_or_else(|| match app.input_mode {
                    InputMode::CreatingDirectory => {
                        "Enter a directory name and press Enter.".to_string()
                    }
                    InputMode::RenamingFileEntry => "Enter a new name and press Enter.".to_string(),
                    InputMode::MovingFileEntry => {
                        "Enter a destination path like archive/note.md.".to_string()
                    }
                    InputMode::CopyingFileEntry => {
                        "Enter a destination path like archive/note-copy.md.".to_string()
                    }
                    _ => format!(
                        "Template: {}. Enter a file name and press Enter.",
                        app.selected_file_template_name()
                    ),
                }),
        )
        .style(if app.file_form_message.is_some() {
            ui_style::danger_style()
        } else {
            ui_style::subtle_style()
        })
        .block(ui_style::popup_block("Feedback", Accent::Notes));
    f.render_widget(feedback, layout[4]);
}

fn draw_delete_file_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
    let popup_area = ui_style::popup_rect(PopupSize::Compact, size);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(3),
        ])
        .split(popup_area);

    let title = Paragraph::new("Delete File Entry")
        .style(ui_style::title_style(Accent::Notes))
        .alignment(tui::layout::Alignment::Center)
        .block(ui_style::popup_block("Delete Entry", Accent::Notes));
    f.render_widget(title, layout[0]);

    let target = app
        .pending_file_path
        .as_ref()
        .map(|path| {
            path.strip_prefix(&app.notes_root)
                .map(|relative| format!("/{}", relative.display()))
                .unwrap_or_else(|_| path.display().to_string())
        })
        .unwrap_or_else(|| "Unknown entry".to_string());
    let message = Paragraph::new(format!(
        "Delete {target}?\nDirectories are removed recursively."
    ))
    .style(ui_style::danger_style())
    .alignment(tui::layout::Alignment::Center)
    .block(ui_style::popup_block("Confirmation", Accent::Notes));
    f.render_widget(message, layout[1]);

    let instructions = Paragraph::new("Press [Y] to confirm or [N] to cancel")
        .style(ui_style::info_style())
        .alignment(tui::layout::Alignment::Center)
        .block(ui_style::popup_block("Controls", Accent::Notes));
    f.render_widget(instructions, layout[2]);
}

fn draw_inline_file_editor<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new(
        app.viewed_file_path
            .as_ref()
            .map(|path| format!("Editing {}", app.relative_path_from_root(path)))
            .unwrap_or_else(|| "Editing file".to_string()),
    )
    .style(ui_style::title_style(Accent::Notes))
    .block(ui_style::surface_block("Inline Editor", Accent::Notes));
    f.render_widget(title, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(chunks[1]);

    let editor_height = content_chunks[0].height.saturating_sub(2) as usize;
    let editor_width = content_chunks[0].width.saturating_sub(2 + 4) as usize;
    app.ensure_file_edit_cursor_visible(editor_height.max(1), editor_width.max(1));
    let lines = app.inline_editor_lines();
    let start = app.file_edit_scroll.min(lines.len().saturating_sub(1));
    let visible_lines = lines
        .iter()
        .skip(start)
        .take(editor_height.max(1))
        .enumerate()
        .map(|(offset, line)| {
            let mut spans = vec![Span::styled(
                format!("{:>3} ", start + offset + 1),
                ui_style::subtle_style(),
            )];
            spans.extend(
                markdown_source_spans(&slice_line_for_view(
                    line,
                    app.file_edit_scroll_x,
                    editor_width.max(1),
                ))
                .0,
            );
            Spans::from(spans)
        })
        .collect::<Vec<_>>();

    let editor = Paragraph::new(visible_lines)
        .style(ui_style::body_style())
        .block(ui_style::surface_block("Markdown", Accent::Notes))
        .wrap(Wrap { trim: false });
    f.render_widget(editor, content_chunks[0]);

    let preview = Paragraph::new(app.inline_editor_preview())
        .style(ui_style::body_style())
        .block(ui_style::surface_block("Live Preview", Accent::Notes))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, content_chunks[1]);

    let feedback = Paragraph::new(app.file_edit_message.clone().unwrap_or_else(|| {
        format!(
            "Ln {}, Col {} | Top {} | Ctrl+S save, Esc cancel.",
            app.file_edit_cursor_row + 1,
            app.file_edit_cursor_col + 1,
            app.file_edit_scroll + 1
        )
    }))
    .style(if app.file_edit_message.is_some() {
        ui_style::success_style()
    } else {
        ui_style::subtle_style()
    })
    .block(ui_style::surface_block("Feedback", Accent::Notes));
    f.render_widget(feedback, chunks[2]);

    let visible_row = app
        .file_edit_cursor_row
        .saturating_sub(app.file_edit_scroll);
    if visible_row < editor_height.max(1) {
        let visible_col = app
            .file_edit_cursor_col
            .saturating_sub(app.file_edit_scroll_x);
        let x = content_chunks[0].x + 1 + 4 + visible_col as u16;
        let y = content_chunks[0].y + 1 + visible_row as u16;
        f.set_cursor(x, y);
    }
}

fn draw_file_shortcuts_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
    let popup_area = ui_style::popup_rect(PopupSize::Standard, size);
    f.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = if app.all_file_shortcuts().is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No shortcuts yet. Use 'm' to pin dirs or save a search with 'S' in find mode.",
            ui_style::muted_style(),
        ))])]
    } else {
        app.all_file_shortcuts()
            .iter()
            .map(|shortcut| {
                let kind = match shortcut.kind {
                    crate::notes::app::FileShortcutKind::Directory => "Pinned Dir",
                    crate::notes::app::FileShortcutKind::Search => "Saved Search",
                };
                ListItem::new(vec![
                    Spans::from(Span::styled(
                        shortcut.name.clone(),
                        ui_style::title_style(Accent::Notes),
                    )),
                    Spans::from(Span::styled(
                        format!("{kind} | {}", shortcut.target),
                        ui_style::muted_style(),
                    )),
                ])
            })
            .collect()
    };

    let list = List::new(items)
        .block(ui_style::popup_block(
            "File Shortcuts (Enter open, x delete)",
            Accent::Notes,
        ))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    if !app.all_file_shortcuts().is_empty() {
        state.select(Some(app.file_shortcut_selected));
    }
    f.render_stateful_widget(list, popup_area, &mut state);
}

fn draw_file_links_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
    let popup_area = ui_style::popup_rect(PopupSize::Wide, size);
    f.render_widget(Clear, popup_area);

    let links = app.related_file_links();
    let items: Vec<ListItem> = if links.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No references or backlinks for this note.",
            ui_style::muted_style(),
        ))])]
    } else {
        links
            .iter()
            .map(|link| {
                ListItem::new(vec![
                    Spans::from(Span::styled(
                        format!("{} | {}", link.group, link.label),
                        ui_style::title_style(Accent::Notes),
                    )),
                    Spans::from(Span::styled(
                        link.path.display().to_string(),
                        ui_style::subtle_style(),
                    )),
                ])
            })
            .collect()
    };

    let list = List::new(items)
        .block(ui_style::popup_block(
            "Related Notes (Enter open, Esc close)",
            Accent::Notes,
        ))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    if !links.is_empty() {
        state.select(Some(app.file_link_selected));
    }
    f.render_stateful_widget(list, popup_area, &mut state);
}
