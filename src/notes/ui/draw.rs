use crate::notes::app::{App, InputMode, NotesView};
use crate::ui_style::{self, Accent, PopupSize};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use super::events::visible_notes_palette_commands;

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

pub fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            [
                Constraint::Min(10),
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Length(10),
            ]
            .as_ref(),
        )
        .split(size);

    match app.input_mode {
        InputMode::ViewingNote => draw_note_view(f, app, chunks[0]),
        InputMode::ViewingFile => draw_file_view(f, app, chunks[0]),
        InputMode::EditingFile => draw_inline_file_editor(f, app, chunks[0]),
        InputMode::AddingNote | InputMode::EditingNote => draw_edit_popup(f, app),
        _ => {
            if app.active_view == NotesView::Files {
                draw_file_browser(f, app, chunks[0]);
            } else {
                draw_notes_list(f, app, chunks[0]);
            }
        }
    }

    let command_lines = match app.input_mode {
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
        InputMode::DeletingFileEntry | InputMode::DeleteNote => {
            vec![ui_style::command_bar_spans(&[
                ("y", "confirm delete"),
                ("n", "cancel"),
            ])]
        }
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

    if app.input_mode == InputMode::Help {
        draw_help_popup(f, size);
    }
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
    .style(ui_style::subtle_style())
    .block(ui_style::surface_block("Scroll", Accent::Notes));
    f.render_widget(scroll_status, chunks[3]);

    let related_items: Vec<ListItem> = if related_links.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No references or backlinks.",
            ui_style::subtle_style(),
        ))])]
    } else {
        related_links
            .iter()
            .map(|link| {
                ListItem::new(vec![Spans::from(vec![
                    Span::styled(
                        format!("{} | ", link.group),
                        ui_style::muted_style().add_modifier(Modifier::BOLD),
                    ),
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
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(3),
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

    if app.editing_title {
        f.set_cursor(
            popup_layout[1].x + app.title_input.len() as u16 + 1,
            popup_layout[1].y + 1,
        );
    } else {
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
