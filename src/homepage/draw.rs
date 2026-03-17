use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::ui_style::{self, Accent};

use super::data::{support_lines_for_leadership, support_lines_for_notes, support_lines_for_tasks};
use super::{AppTool, HomepageDashboard};

pub fn draw_homepage<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    tools: &[AppTool],
    selected: usize,
    dashboard: &HomepageDashboard,
    error_message: Option<&str>,
) {
    let size = f.size();
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(20),
            Constraint::Length(4),
        ])
        .split(size);

    draw_header(f, outer[0], dashboard, error_message);
    draw_dashboard(f, outer[1], tools, selected, dashboard);
    draw_footer(f, outer[2]);
}

fn draw_header<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    area: Rect,
    dashboard: &HomepageDashboard,
    error_message: Option<&str>,
) {
    let title_lines = vec![
        Spans::from(Span::styled(
            "Workspace Control Center",
            ui_style::title_style(Accent::Primary),
        )),
        Spans::from(Span::styled(
            format!(
                "Tasks: {} total | Notes DB: {} | Note Files: {}",
                dashboard.tasks.task_count,
                dashboard.notes.db_note_count,
                dashboard.notes.file_count,
            ),
            ui_style::info_style(),
        )),
        Spans::from(Span::styled(
            format!(
                "EM Tools: {} records | Refreshed: {}",
                dashboard.one_on_ones.count
                    + dashboard.delegations.count
                    + dashboard.decisions.count,
                dashboard.refreshed_at
            ),
            ui_style::muted_style(),
        )),
        Spans::from(Span::styled(
            error_message.unwrap_or("Enter launches the selected app. r refreshes this dashboard."),
            if error_message.is_some() {
                ui_style::danger_style()
            } else {
                ui_style::muted_style()
            },
        )),
    ];

    let header = Paragraph::new(title_lines)
        .block(ui_style::shell_block("Homepage"))
        .wrap(Wrap { trim: false });
    f.render_widget(header, area);
}

fn draw_dashboard<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    area: Rect,
    tools: &[AppTool],
    selected: usize,
    dashboard: &HomepageDashboard,
) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_tool_launcher(f, columns[0], tools, selected, dashboard);
    draw_detail_panels(f, columns[1], tools[selected], dashboard);
}

fn draw_tool_launcher<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    area: Rect,
    tools: &[AppTool],
    selected: usize,
    dashboard: &HomepageDashboard,
) {
    let items: Vec<ListItem> = tools
        .iter()
        .map(|tool| {
            let metric = match tool {
                AppTool::TaskManager => format!(
                    "{} tasks | {} open",
                    dashboard.tasks.task_count, dashboard.tasks.open_count,
                ),
                AppTool::Notes => format!(
                    "{} notes | {} files",
                    dashboard.notes.db_note_count, dashboard.notes.file_count,
                ),
                AppTool::OneOnOneManager => format!(
                    "{} 1:1s | {} scheduled",
                    dashboard.one_on_ones.count, dashboard.one_on_ones.stat_a_value
                ),
                AppTool::DelegationTracker => format!(
                    "{} items | {} open",
                    dashboard.delegations.count, dashboard.delegations.stat_a_value
                ),
                AppTool::DecisionLog => format!(
                    "{} decisions | {} decided",
                    dashboard.decisions.count, dashboard.decisions.stat_a_value
                ),
            };
            ListItem::new(vec![
                Spans::from(Span::styled(
                    tool.title(),
                    Style::default()
                        .fg(tool.accent())
                        .add_modifier(Modifier::BOLD),
                )),
                Spans::from(Span::styled(tool.subtitle(), ui_style::body_style())),
                Spans::from(Span::styled(metric, ui_style::subtle_style())),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(ui_style::surface_block("Launcher", Accent::Primary))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    state.select(Some(selected));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_detail_panels<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    area: Rect,
    selected_tool: AppTool,
    dashboard: &HomepageDashboard,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(9),
            Constraint::Length(6),
        ])
        .split(area);

    draw_selected_tool_summary(f, rows[0], selected_tool, dashboard);

    match selected_tool {
        AppTool::TaskManager => draw_recent_panel(
            f,
            rows[1],
            "Recent Tasks",
            &dashboard.tasks.recent_tasks,
            Color::Yellow,
        ),
        AppTool::Notes => {
            let middle = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(rows[1]);
            draw_recent_panel(
                f,
                middle[0],
                "Recent Notes",
                &dashboard.notes.recent_notes,
                Color::Cyan,
            );
            draw_recent_panel(
                f,
                middle[1],
                "Recent Files",
                &dashboard.notes.recent_files,
                Color::Green,
            );
        }
        AppTool::OneOnOneManager => draw_recent_panel(
            f,
            rows[1],
            "Recent 1:1s",
            &dashboard.one_on_ones.recent_items,
            ui_style::accent_color(Accent::LeadershipPeople),
        ),
        AppTool::DelegationTracker => draw_recent_panel(
            f,
            rows[1],
            "Recent Delegations",
            &dashboard.delegations.recent_items,
            ui_style::accent_color(Accent::Delegation),
        ),
        AppTool::DecisionLog => draw_recent_panel(
            f,
            rows[1],
            "Recent Decisions",
            &dashboard.decisions.recent_items,
            ui_style::accent_color(Accent::Decisions),
        ),
    }

    let support = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);

    match selected_tool {
        AppTool::TaskManager => {
            let [left, right] = support_lines_for_tasks(dashboard);
            draw_paths_panel(f, support[0], "Task Paths", &left);
            draw_notes_snapshot(f, support[1], &right);
        }
        AppTool::Notes => {
            let [left, right] = support_lines_for_notes(dashboard);
            draw_notes_snapshot(f, support[0], &left);
            draw_paths_panel(f, support[1], "Task Context", &right);
        }
        AppTool::OneOnOneManager => {
            let [left, right] = support_lines_for_leadership(
                &dashboard.one_on_ones,
                20,
                [
                    format!("Task backlog: {}", dashboard.tasks.open_count),
                    format!("Notes files: {}", dashboard.notes.file_count),
                    "Use for prep, follow-ups, and private reminders.".to_string(),
                ],
            );
            draw_paths_panel(f, support[0], "1:1 Metrics", &left);
            draw_notes_snapshot(f, support[1], &right);
        }
        AppTool::DelegationTracker => {
            let [left, right] = support_lines_for_leadership(
                &dashboard.delegations,
                20,
                [
                    format!("Task backlog: {}", dashboard.tasks.open_count),
                    format!("Recent notes: {}", dashboard.notes.recent_notes.len()),
                    "Use for follow-through and owner accountability.".to_string(),
                ],
            );
            draw_paths_panel(f, support[0], "Delegation Metrics", &left);
            draw_notes_snapshot(f, support[1], &right);
        }
        AppTool::DecisionLog => {
            let [left, right] = support_lines_for_leadership(
                &dashboard.decisions,
                20,
                [
                    format!("Task backlog: {}", dashboard.tasks.open_count),
                    format!("Notes DB: {}", dashboard.notes.db_note_count),
                    "Use for rationale, impact, and later review.".to_string(),
                ],
            );
            draw_paths_panel(f, support[0], "Decision Metrics", &left);
            draw_notes_snapshot(f, support[1], &right);
        }
    }
}

fn draw_selected_tool_summary<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    area: Rect,
    selected_tool: AppTool,
    dashboard: &HomepageDashboard,
) {
    let (title, body, accent) = match selected_tool {
        AppTool::TaskManager => (
            "Task Manager Snapshot",
            vec![
                Spans::from(vec![
                    Span::styled("Open: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.tasks.open_count.to_string(),
                        ui_style::warning_style(),
                    ),
                    Span::raw("    "),
                    Span::styled("Done: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.tasks.done_count.to_string(),
                        ui_style::success_style(),
                    ),
                ]),
                Spans::from(vec![
                    Span::styled("Topics: ", ui_style::muted_style()),
                    Span::raw(dashboard.tasks.topic_count.to_string()),
                    Span::raw("    "),
                    Span::styled("Favourites: ", ui_style::muted_style()),
                    Span::raw(dashboard.tasks.favourite_count.to_string()),
                ]),
                Spans::from(Span::styled(
                    "Focused execution queue with topic-level structure.",
                    ui_style::body_style(),
                )),
            ],
            Accent::Tasks,
        ),
        AppTool::Notes => (
            "Notes Snapshot",
            vec![
                Spans::from(vec![
                    Span::styled("DB Notes: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.notes.db_note_count.to_string(),
                        ui_style::title_style(Accent::Notes),
                    ),
                    Span::raw("    "),
                    Span::styled("Files: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.notes.file_count.to_string(),
                        ui_style::success_style(),
                    ),
                ]),
                Spans::from(vec![
                    Span::styled("Directories: ", ui_style::muted_style()),
                    Span::raw(dashboard.notes.directory_count.to_string()),
                    Span::raw("    "),
                    Span::styled("Recent DB items: ", ui_style::muted_style()),
                    Span::raw(dashboard.notes.recent_notes.len().to_string()),
                ]),
                Spans::from(Span::styled(
                    "File-first navigation, markdown editing, linked references.",
                    ui_style::body_style(),
                )),
            ],
            Accent::Notes,
        ),
        AppTool::OneOnOneManager => (
            "1:1 Snapshot",
            vec![
                Spans::from(vec![
                    Span::styled("Total: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.one_on_ones.count.to_string(),
                        ui_style::title_style(Accent::LeadershipPeople),
                    ),
                    Span::raw("    "),
                    Span::styled("Scheduled: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.one_on_ones.stat_a_value.to_string(),
                        ui_style::success_style(),
                    ),
                ]),
                Spans::from(vec![
                    Span::styled(
                        format!("{}: ", title_case_label(dashboard.one_on_ones.stat_b_label)),
                        ui_style::muted_style(),
                    ),
                    Span::raw(dashboard.one_on_ones.stat_b_value.to_string()),
                ]),
                Spans::from(Span::styled(
                    "Track people context, prep, and commitments.",
                    ui_style::body_style(),
                )),
            ],
            Accent::LeadershipPeople,
        ),
        AppTool::DelegationTracker => (
            "Delegation Snapshot",
            vec![
                Spans::from(vec![
                    Span::styled("Total: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.delegations.count.to_string(),
                        ui_style::title_style(Accent::Delegation),
                    ),
                    Span::raw("    "),
                    Span::styled("Open: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.delegations.stat_a_value.to_string(),
                        ui_style::warning_style(),
                    ),
                ]),
                Spans::from(vec![
                    Span::styled(
                        format!("{}: ", title_case_label(dashboard.delegations.stat_b_label)),
                        ui_style::muted_style(),
                    ),
                    Span::raw(dashboard.delegations.stat_b_value.to_string()),
                ]),
                Spans::from(Span::styled(
                    "Track delegated work, ownership, and follow-through.",
                    ui_style::body_style(),
                )),
            ],
            Accent::Delegation,
        ),
        AppTool::DecisionLog => (
            "Decision Snapshot",
            vec![
                Spans::from(vec![
                    Span::styled("Total: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.decisions.count.to_string(),
                        ui_style::title_style(Accent::Decisions),
                    ),
                    Span::raw("    "),
                    Span::styled("Decided: ", ui_style::muted_style()),
                    Span::styled(
                        dashboard.decisions.stat_a_value.to_string(),
                        ui_style::success_style(),
                    ),
                ]),
                Spans::from(vec![
                    Span::styled(
                        format!("{}: ", title_case_label(dashboard.decisions.stat_b_label)),
                        ui_style::muted_style(),
                    ),
                    Span::raw(dashboard.decisions.stat_b_value.to_string()),
                ]),
                Spans::from(Span::styled(
                    "Capture rationale, impact, and later reference.",
                    ui_style::body_style(),
                )),
            ],
            Accent::Decisions,
        ),
    };

    let summary = Paragraph::new(body)
        .block(ui_style::surface_block(title, accent))
        .wrap(Wrap { trim: false });
    f.render_widget(summary, area);
}

fn title_case_label(label: &str) -> String {
    label
        .split([' ', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_uppercase().collect::<String>();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn draw_recent_panel<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    area: Rect,
    title: &str,
    items: &[String],
    accent: Color,
) {
    let lines: Vec<Spans> = if items.is_empty() {
        vec![Spans::from(Span::styled(
            "No items yet.",
            ui_style::subtle_style(),
        ))]
    } else {
        items
            .iter()
            .map(|item| {
                Spans::from(vec![
                    Span::styled("• ", Style::default().fg(accent)),
                    Span::styled(item.clone(), ui_style::body_style()),
                ])
            })
            .collect()
    };

    let panel = Paragraph::new(lines)
        .block(ui_style::shell_block(title))
        .wrap(Wrap { trim: false });
    f.render_widget(panel, area);
}

fn draw_paths_panel<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    area: Rect,
    title: &str,
    lines: &[String],
) {
    let spans = lines
        .iter()
        .map(|line| Spans::from(Span::styled(line.clone(), ui_style::body_style())))
        .collect::<Vec<_>>();

    let panel = Paragraph::new(spans)
        .block(ui_style::shell_block(title))
        .wrap(Wrap { trim: false });
    f.render_widget(panel, area);
}

fn draw_notes_snapshot<B: tui::backend::Backend>(
    f: &mut tui::Frame<B>,
    area: Rect,
    lines: &[String],
) {
    let spans = lines
        .iter()
        .map(|line| Spans::from(Span::styled(line.clone(), ui_style::muted_style())))
        .collect::<Vec<_>>();
    let panel = Paragraph::new(spans)
        .block(ui_style::shell_block("Context"))
        .wrap(Wrap { trim: false });
    f.render_widget(panel, area);
}

fn draw_footer<B: tui::backend::Backend>(f: &mut tui::Frame<B>, area: Rect) {
    let footer = Paragraph::new(vec![
        ui_style::command_bar_spans(&[
            ("Enter", "launch selected app"),
            ("j/k", "move"),
            ("↑/↓", "move"),
            ("r", "refresh dashboard"),
            ("q", "quit"),
        ]),
        Spans::from(Span::styled(
            "Homepage is read-only. Launch a tool to edit data.",
            ui_style::muted_style(),
        )),
    ])
    .block(ui_style::command_bar_block("Commands"));
    f.render_widget(Clear, area);
    f.render_widget(footer, area);
}
