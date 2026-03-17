use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::text::{Span, Spans};
use tui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use tui::Frame;

use crate::common::widgets;
use crate::leadership_tools::app::{App, InputMode, LinkedRecordKind, ToolKind};
use crate::ui_style::{self, PopupSize};

pub(super) fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(12),
            Constraint::Length(6),
            Constraint::Length(7),
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
        InputMode::Filtering => {}
        InputMode::Editing => draw_edit_popup(f, app),
        InputMode::DeleteConfirm => draw_delete_popup(f, app),
        InputMode::LinkedRecords => draw_link_popup(f, app),
        InputMode::Normal => {}
    }
}

fn draw_list<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let filtered_indices = app.filtered_indices();
    let list_title = if app.has_filter() {
        format!(
            "{} [{} / {}] | Filter: {}",
            app.spec.list_title,
            filtered_indices.len(),
            app.records.len(),
            app.filter_query
        )
    } else {
        format!("{} [{}]", app.spec.list_title, app.records.len())
    };
    let items = if app.records.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No records yet. Press 'a' to add one.",
            ui_style::muted_style(),
        ))])]
    } else if filtered_indices.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            format!("No records match \"{}\".", app.filter_query),
            ui_style::muted_style(),
        ))])]
    } else {
        filtered_indices
            .iter()
            .filter_map(|index| app.records.get(*index))
            .map(|record| {
                let values = app.spec.normalize_values(&record.values);
                let title = values.first().cloned().unwrap_or_default();
                let summary = app.spec.list_summary(&values);
                let summary_style = if matches!(app.spec.kind, ToolKind::Delegation) {
                    if app.spec.is_overdue(&values) {
                        ui_style::danger_style()
                    } else if app.spec.needs_follow_up(&values) {
                        ui_style::warning_style()
                    } else {
                        ui_style::muted_style()
                    }
                } else if matches!(app.spec.kind, ToolKind::Decision) {
                    if app.spec.review_due(&values) {
                        ui_style::warning_style()
                    } else {
                        ui_style::muted_style()
                    }
                } else {
                    ui_style::muted_style()
                };
                ListItem::new(vec![
                    Spans::from(Span::styled(title, ui_style::title_style(app.spec.accent))),
                    Spans::from(Span::styled(summary, summary_style)),
                ])
            })
            .collect()
    };

    let list = List::new(items)
        .block(ui_style::surface_block(&list_title, app.spec.accent))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    if !filtered_indices.is_empty() {
        state.select(
            filtered_indices
                .iter()
                .position(|index| *index == app.selected),
        );
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
                ui_style::command_bar_spans(&[("/", "filter"), ("q", "back to homepage")]),
            ];
            if matches!(app.spec.kind, ToolKind::Decision) {
                lines[0] = ui_style::command_bar_spans(&[
                    ("a", "add"),
                    ("e", "edit"),
                    ("d", "delete"),
                    ("o", "open links"),
                ]);
            }
            if app.spec.statuses().is_some() {
                if matches!(app.spec.kind, ToolKind::Delegation) {
                    lines[1] = ui_style::command_bar_spans(&[
                        ("t", "cycle status"),
                        ("r", "log reminder"),
                    ]);
                    lines.push(ui_style::command_bar_spans(&[
                        ("/", "filter"),
                        ("q", "back"),
                    ]));
                } else if matches!(app.spec.kind, ToolKind::Decision) {
                    lines[1] = ui_style::command_bar_spans(&[
                        ("t", "cycle status"),
                        ("v", "schedule review"),
                    ]);
                    lines.push(ui_style::command_bar_spans(&[
                        ("/", "filter"),
                        ("q", "back"),
                    ]));
                } else {
                    lines[1] = ui_style::command_bar_spans(&[
                        ("t", "cycle status"),
                        ("/", "filter"),
                        ("q", "back"),
                    ]);
                }
            } else if matches!(app.spec.kind, ToolKind::OneOnOne) {
                lines[1] = ui_style::command_bar_spans(&[
                    ("n", "advance by cadence"),
                    ("m", "complete meeting"),
                ]);
                lines.push(ui_style::command_bar_spans(&[
                    ("x", "extract actions"),
                    ("/", "filter"),
                ]));
                lines.push(ui_style::command_bar_spans(&[("q", "back")]));
            }
            lines
        }
        InputMode::Filtering => vec![
            Spans::from(vec![
                Span::raw("Filter "),
                Span::styled(
                    app.filter_query.clone(),
                    ui_style::title_style(app.spec.accent),
                ),
            ]),
            filter_help_spans(app),
        ],
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
        InputMode::LinkedRecords => vec![
            Spans::from(Span::styled(
                "Linked records on this decision",
                ui_style::title_style(app.spec.accent),
            )),
            ui_style::command_bar_spans(&[("j/k", "move"), ("Enter", "open"), ("Esc", "close")]),
        ],
    };

    let panel = Paragraph::new(command_lines)
        .style(ui_style::info_style())
        .block(ui_style::command_bar_block("Commands"));
    f.render_widget(panel, area);
}

fn filter_help_spans(app: &App) -> Spans<'static> {
    match app.spec.kind {
        ToolKind::OneOnOne => ui_style::command_bar_spans(&[
            ("Enter", "keep"),
            ("Esc", "clear"),
            ("person:/relationship:/type:/team:", "tokens"),
            ("manager:/purpose:/actions", "workflow"),
        ]),
        ToolKind::Delegation => ui_style::command_bar_spans(&[
            ("Enter", "keep"),
            ("Esc", "clear"),
            ("owner:/status:/due:/followup:", "tokens"),
            ("overdue/reminders", "action queue"),
        ]),
        ToolKind::Decision => ui_style::command_bar_spans(&[
            ("Enter", "keep"),
            ("Esc", "clear"),
            ("owner:/status:/date:/tag:", "tokens"),
            ("note:/task:/review", "links"),
        ]),
    }
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
    let message = app
        .selected_record()
        .and_then(|record| record.values.first().cloned())
        .map(|value| format!("Delete \"{value}\"?"))
        .unwrap_or_else(|| "Delete selected record?".to_string());
    widgets::draw_confirmation_popup(
        f,
        f.size(),
        app.spec.accent,
        "Delete",
        "Delete Record",
        &message,
        "Press [Y] to confirm or [N] to cancel",
    );
}

fn draw_link_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let items = if app.linked_records.is_empty() {
        vec![ListItem::new(Spans::from(Span::styled(
            "No linked records resolved.",
            ui_style::muted_style(),
        )))]
    } else {
        app.linked_records
            .iter()
            .map(|record| {
                let accent = match record.kind {
                    LinkedRecordKind::Task => crate::ui_style::Accent::Tasks,
                    LinkedRecordKind::Note => crate::ui_style::Accent::Notes,
                };
                let kind_label = match record.kind {
                    LinkedRecordKind::Task => "Task",
                    LinkedRecordKind::Note => "Note",
                };
                ListItem::new(vec![
                    Spans::from(Span::styled(
                        format!("{kind_label} #{} - {}", record.id, record.title),
                        ui_style::title_style(accent),
                    )),
                    Spans::from(Span::styled(
                        record.summary.clone(),
                        ui_style::muted_style(),
                    )),
                ])
            })
            .collect()
    };
    widgets::draw_list_popup(
        f,
        f.size(),
        PopupSize::Wide,
        app.spec.accent,
        "Linked Records",
        items,
        if app.linked_records.is_empty() {
            None
        } else {
            Some(app.linked_record_selected)
        },
    );
}
