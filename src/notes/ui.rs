use crate::notes::app::{App, InputMode};
use crossterm::event::{self, Event, KeyCode};
use std::time::{Duration, Instant};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
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
                        KeyCode::Char('p') => {
                            app.begin_note_presets();
                        }
                        KeyCode::Char('/') => {
                            app.begin_note_filter();
                        }
                        KeyCode::Char('a') => {
                            app.begin_add_note();
                        }
                        KeyCode::Char('e') => {
                            app.begin_edit_note();
                        }
                        KeyCode::Char('d') => {
                            app.begin_delete_note();
                        }
                        KeyCode::Enter => {
                            if !app.notes.is_empty() {
                                app.input_mode = InputMode::ViewingNote;
                            }
                        }
                        KeyCode::Char('H') => {
                            app.input_mode = InputMode::Help;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.move_selection_down();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.move_selection_up();
                        }
                        KeyCode::PageUp => {
                            app.log_offset += 1;
                        }
                        KeyCode::PageDown => {
                            if app.log_offset > 0 {
                                app.log_offset -= 1;
                            }
                        }
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
                        KeyCode::Enter => {
                            app.apply_selected_note_preset();
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let len = app.note_filter_presets().len();
                            app.move_preset_down(len);
                        }
                        KeyCode::Up | KeyCode::Char('k') => app.move_preset_up(),
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
                        KeyCode::Esc | KeyCode::Enter => {
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
                Constraint::Length(3),  // Instructions
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
        InputMode::AddingNote | InputMode::EditingNote => {
            draw_edit_popup(f, app);
        }
        _ => {
            draw_notes_list(f, app, chunks[0]);
        }
    }

    // Instructions
    let help_hint = Spans::from(vec![
        Span::raw("Press "),
        Span::styled("H", Style::default().fg(Color::Yellow)),
        Span::raw(" for help."),
    ]);
    let input_msg = match app.input_mode {
        InputMode::Filtering => Spans::from(vec![
            Span::raw("Filter notes: "),
            Span::styled(app.note_filter.as_str(), Style::default().fg(Color::Yellow)),
            Span::raw(" (Enter to keep, Esc to clear. Tokens: title:, body:, quotes, -negation)"),
        ]),
        InputMode::AddingNote => Spans::from(vec![Span::raw(
            "Adding note. Enter title, then content. Press Enter to save, Esc to cancel.",
        )]),
        InputMode::EditingNote => Spans::from(vec![Span::raw(
            "Editing note. Press Enter to save, Esc to cancel.",
        )]),
        InputMode::DeleteNote => Spans::from(vec![Span::raw(
            "Press [Y] to confirm deletion or [N] to cancel.",
        )]),
        _ => help_hint,
    };
    let help_message = Paragraph::new(input_msg)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Instructions"));
    f.render_widget(help_message, chunks[1]);

    // Mode indicator
    let mode_text = match app.input_mode {
        InputMode::Normal => "Normal Mode",
        InputMode::Filtering => "Filtering Notes",
        InputMode::PresetFilters => "Note Presets",
        InputMode::AddingNote => "Adding Note",
        InputMode::EditingNote => "Editing Note",
        InputMode::ViewingNote => "Viewing Note",
        InputMode::DeleteNote => "Delete Note",
        InputMode::Help => "Viewing Help",
    };
    let mode =
        Paragraph::new(mode_text).block(Block::default().borders(Borders::ALL).title("Mode"));
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
    let logs_list =
        List::new(visible_logs).block(Block::default().borders(Borders::ALL).title("Logs"));
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
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
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
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                        Style::default().bg(Color::Yellow).fg(Color::Black),
                    ),
                    highlighted_spans(
                        &preview,
                        &app.note_filter,
                        Style::default().fg(Color::Gray),
                        Style::default().bg(Color::Yellow).fg(Color::Black),
                    ),
                    Spans::from(Span::styled(
                        format!(
                            "Created: {} | Updated: {}",
                            note.created_at, note.updated_at
                        ),
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
            })
            .collect()
    };

    let notes_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if app.has_note_filter() {
                    format!("Notes | Filter: {}", app.note_filter)
                } else {
                    "Notes".to_string()
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
    f.render_stateful_widget(notes_list, area, &mut list_state);
}

fn draw_note_view<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    if let Some(note) = app.notes.get(app.selected) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(area);

        let title = Paragraph::new(note.title.clone())
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL).title("Title"));
        f.render_widget(title, chunks[0]);

        let content = Paragraph::new(note.content.clone())
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Content"))
            .wrap(Wrap { trim: true });
        f.render_widget(content, chunks[1]);
    }
}

fn draw_edit_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let popup_area = centered_rect(70, 70, size);
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
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(popup_title, popup_layout[0]);

    let title_style = if app.editing_title {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let title_input = Paragraph::new(app.title_input.as_ref())
        .style(title_style)
        .block(Block::default().borders(Borders::ALL).title("Title"));
    f.render_widget(title_input, popup_layout[1]);

    let content_style = if app.editing_title {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().fg(Color::White)
    };
    let content_input = Paragraph::new(app.content_input.as_ref())
        .style(content_style)
        .block(Block::default().borders(Borders::ALL).title("Content"))
        .wrap(Wrap { trim: true });
    f.render_widget(content_input, popup_layout[2]);

    let feedback_text = app
        .note_form_message
        .clone()
        .unwrap_or_else(|| "Enter a title, then content, then save.".to_string());
    let feedback_style = if app.note_form_message.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let feedback = Paragraph::new(feedback_text)
        .style(feedback_style)
        .block(Block::default().borders(Borders::ALL).title("Feedback"));
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
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::ALL));
    f.render_widget(instructions, popup_layout[4]);
}

fn draw_delete_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
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

    let note_name = if let Some(note) = app.notes.get(app.selected) {
        &note.title
    } else {
        "Unknown Note"
    };

    let delete_msg = Paragraph::new(format!(
        "Are you sure you want to delete \"{}\"?",
        note_name
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

fn draw_help_popup<B: Backend>(f: &mut Frame<B>, size: Rect) {
    let help_area = centered_rect(60, 70, size);
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
            "Navigation:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from("  Up/Down or j/k - Navigate notes"),
        Spans::from("  Enter - View selected note"),
        Spans::from(""),
        Spans::from(Span::styled(
            "Actions:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from("  a - Add new note"),
        Spans::from("  e - Edit selected note"),
        Spans::from("  d - Delete selected note"),
        Spans::from("  / - Filter notes (supports title:, body:, quotes, and -negation)"),
        Spans::from("  p - Open preset note filters"),
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
    ];

    let help_paragraph = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .wrap(Wrap { trim: true });
    f.render_widget(help_paragraph, help_area);
}

fn draw_note_presets_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
    let popup_area = centered_rect(55, 45, size);
    f.render_widget(Clear, popup_area);

    let presets = app.note_filter_presets();
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
        .block(Block::default().borders(Borders::ALL).title("Note Presets"))
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
