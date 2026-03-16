use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::error::Error;
use std::time::{Duration, Instant};
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::text::{Span, Spans};
use tui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use tui::{Frame, Terminal};

use crate::leadership_tools::app::{App, InputMode};
use crate::ui_style::{self, PopupSize};

pub fn run<B: Backend>(app: &mut App, terminal: &mut Terminal<B>) -> Result<(), Box<dyn Error>> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    terminal.clear()?;

    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if handle_key(app, key)? {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool, Box<dyn Error>> {
    match app.input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('j') | KeyCode::Down => app.move_selection_down(),
            KeyCode::Char('k') | KeyCode::Up => app.move_selection_up(),
            KeyCode::Char('a') => app.begin_add(),
            KeyCode::Char('e') => app.begin_edit(),
            KeyCode::Char('d') => app.begin_delete(),
            KeyCode::Char('t') => app.cycle_status()?,
            _ => {}
        },
        InputMode::Editing => match key.code {
            KeyCode::Esc => app.cancel_modal(),
            KeyCode::Tab | KeyCode::Down => app.focus_next_field(),
            KeyCode::BackTab | KeyCode::Up => app.focus_prev_field(),
            KeyCode::Enter => app.advance_or_save()?,
            KeyCode::Backspace => app.backspace(),
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if c == 's' {
                    app.save_form()?;
                }
            }
            KeyCode::Char(c) => app.append_char(c),
            _ => {}
        },
        InputMode::DeleteConfirm => match key.code {
            KeyCode::Char('y') => app.delete_selected()?,
            KeyCode::Char('n') | KeyCode::Esc => app.cancel_modal(),
            _ => {}
        },
    }
    Ok(false)
}

pub fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(12),
            Constraint::Length(5),
            Constraint::Length(6),
        ])
        .split(size);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(chunks[0]);

    draw_list(f, app, body[0]);
    draw_detail(f, app, body[1]);
    draw_commands(f, app, chunks[1]);
    draw_logs(f, app, chunks[2]);

    match app.input_mode {
        InputMode::Editing => draw_edit_popup(f, app),
        InputMode::DeleteConfirm => draw_delete_popup(f, app),
        InputMode::Normal => {}
    }
}

fn draw_list<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let list_title = format!("{} [{}]", app.spec.list_title, app.records.len());
    let items = if app.records.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No records yet. Press 'a' to add one.",
            ui_style::muted_style(),
        ))])]
    } else {
        app.list_items()
            .iter()
            .map(|(title, summary)| {
                ListItem::new(vec![
                    Spans::from(Span::styled(
                        title.clone(),
                        ui_style::title_style(app.spec.accent),
                    )),
                    Spans::from(Span::styled(summary.clone(), ui_style::muted_style())),
                ])
            })
            .collect()
    };

    let list = List::new(items)
        .block(ui_style::surface_block(&list_title, app.spec.accent))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    if !app.records.is_empty() {
        state.select(Some(app.selected));
    }
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_detail<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let lines = app
        .detail_lines()
        .into_iter()
        .map(|line| Spans::from(Span::styled(line, ui_style::body_style())))
        .collect::<Vec<_>>();
    let detail = Paragraph::new(lines)
        .block(ui_style::surface_block(
            app.spec.detail_title,
            app.spec.accent,
        ))
        .wrap(Wrap { trim: false });
    f.render_widget(detail, area);
}

fn draw_commands<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let command_lines = match app.input_mode {
        InputMode::Normal => {
            let mut lines = vec![
                ui_style::command_bar_spans(&[
                    ("a", "add"),
                    ("e", "edit"),
                    ("d", "delete"),
                    ("j/k", "move"),
                ]),
                ui_style::command_bar_spans(&[("q", "back to homepage")]),
            ];
            if app.spec.statuses().is_some() {
                lines[1] = ui_style::command_bar_spans(&[("t", "cycle status"), ("q", "back")]);
            }
            lines
        }
        InputMode::Editing => vec![
            Spans::from(vec![
                Span::raw("Editing "),
                Span::styled(
                    app.spec.field_labels[app.editing_field].to_string(),
                    ui_style::title_style(app.spec.accent),
                ),
            ]),
            ui_style::command_bar_spans(&[
                ("Tab", "next field"),
                ("Shift+Tab", "prev"),
                ("Enter", "next/save"),
                ("Ctrl+S", "save"),
                ("Esc", "cancel"),
            ]),
        ],
        InputMode::DeleteConfirm => vec![ui_style::command_bar_spans(&[
            ("y", "confirm delete"),
            ("n", "cancel"),
        ])],
    };

    let panel = Paragraph::new(command_lines)
        .style(ui_style::info_style())
        .block(ui_style::command_bar_block("Commands"));
    f.render_widget(panel, area);
}

fn draw_logs<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let content = app
        .logs
        .iter()
        .map(|line| Spans::from(Span::styled(line.clone(), ui_style::muted_style())))
        .collect::<Vec<_>>();
    let logs = Paragraph::new(content)
        .block(ui_style::shell_block("Activity"))
        .wrap(Wrap { trim: false });
    f.render_widget(logs, area);
}

fn draw_edit_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let popup = ui_style::popup_rect(PopupSize::Wide, f.size());
    f.render_widget(Clear, popup);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(7),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(popup);

    let title = Paragraph::new(if app.editing_index.is_some() {
        format!("Edit {}", app.spec.title)
    } else {
        format!("Add {}", app.spec.title)
    })
    .style(ui_style::title_style(app.spec.accent))
    .block(ui_style::popup_block("Editor", app.spec.accent));
    f.render_widget(title, layout[0]);

    let input = Paragraph::new(app.form_values[app.editing_field].as_str())
        .style(ui_style::body_style())
        .block(ui_style::popup_block(
            app.spec.field_labels[app.editing_field],
            app.spec.accent,
        ));
    f.render_widget(input, layout[1]);
    f.set_cursor(
        layout[1].x + app.form_values[app.editing_field].len() as u16 + 1,
        layout[1].y + 1,
    );

    let draft = Paragraph::new(
        app.draft_lines()
            .into_iter()
            .map(|line| {
                let style = if line.starts_with('>') {
                    ui_style::title_style(app.spec.accent)
                } else {
                    ui_style::muted_style()
                };
                Spans::from(Span::styled(line, style))
            })
            .collect::<Vec<_>>(),
    )
    .block(ui_style::popup_block("Draft", app.spec.accent))
    .wrap(Wrap { trim: false });
    f.render_widget(draft, layout[2]);

    let feedback = Paragraph::new(
        app.feedback
            .clone()
            .unwrap_or_else(|| "Fill the fields, then save.".to_string()),
    )
    .style(if app.feedback.is_some() {
        ui_style::danger_style()
    } else {
        ui_style::subtle_style()
    })
    .block(ui_style::popup_block("Feedback", app.spec.accent));
    f.render_widget(feedback, layout[3]);

    let instructions = Paragraph::new(vec![ui_style::command_bar_spans(&[
        ("Tab", "next"),
        ("Shift+Tab", "prev"),
        ("Enter", "next/save"),
        ("Esc", "cancel"),
    ])])
    .style(ui_style::info_style())
    .block(ui_style::popup_block("Controls", app.spec.accent));
    f.render_widget(instructions, layout[4]);
}

fn draw_delete_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let popup = ui_style::popup_rect(PopupSize::Compact, f.size());
    f.render_widget(Clear, popup);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(popup);

    let title = Paragraph::new("Delete Record")
        .style(ui_style::title_style(app.spec.accent))
        .block(ui_style::popup_block("Delete", app.spec.accent));
    f.render_widget(title, layout[0]);

    let message = Paragraph::new(
        app.selected_record()
            .and_then(|record| record.values.first().cloned())
            .map(|value| format!("Delete \"{value}\"?"))
            .unwrap_or_else(|| "Delete selected record?".to_string()),
    )
    .style(ui_style::danger_style())
    .block(ui_style::popup_block("Confirmation", app.spec.accent));
    f.render_widget(message, layout[1]);

    let controls = Paragraph::new(vec![ui_style::command_bar_spans(&[
        ("y", "confirm"),
        ("n", "cancel"),
    ])])
    .style(ui_style::info_style())
    .block(ui_style::popup_block("Controls", app.spec.accent));
    f.render_widget(controls, layout[2]);
}
