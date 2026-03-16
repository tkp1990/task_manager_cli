use crossterm::event::{self, Event, KeyCode};
use diesel::prelude::*;
use std::fs;
use std::io::Stdout;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use std::{error::Error, io};
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use tui::Terminal;

use crate::db::notes::models::Note;
use crate::db::schema::{note, task, topic};
use crate::db::task_manager::models::{Task, Topic};
use crate::leadership_tools::{
    load_dashboard_snapshot, DashboardSnapshot, ToolKind as LeadershipTool,
};
use crate::ui_style::{self, Accent};

/// Enum representing a tool available from the homepage.
#[derive(Clone, Copy)]
pub enum AppTool {
    TaskManager,
    Notes,
    OneOnOneManager,
    DelegationTracker,
    DecisionLog,
}

impl AppTool {
    pub fn title(&self) -> &'static str {
        match self {
            AppTool::TaskManager => "Task Manager",
            AppTool::Notes => "Notes",
            AppTool::OneOnOneManager => "1:1 Manager",
            AppTool::DelegationTracker => "Delegation Tracker",
            AppTool::DecisionLog => "Decision Log",
        }
    }

    fn subtitle(&self) -> &'static str {
        match self {
            AppTool::TaskManager => "Tasks, topics, favourites.",
            AppTool::Notes => "Files, markdown, links.",
            AppTool::OneOnOneManager => "People, agenda, follow-ups.",
            AppTool::DelegationTracker => "Owners, status, due dates.",
            AppTool::DecisionLog => "Decisions, rationale, impact.",
        }
    }

    fn accent(&self) -> Color {
        match self {
            AppTool::TaskManager => ui_style::accent_color(Accent::Tasks),
            AppTool::Notes => ui_style::accent_color(Accent::Notes),
            AppTool::OneOnOneManager => ui_style::accent_color(Accent::LeadershipPeople),
            AppTool::DelegationTracker => ui_style::accent_color(Accent::Delegation),
            AppTool::DecisionLog => ui_style::accent_color(Accent::Decisions),
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            AppTool::TaskManager => crate::task_manager::run_task_manager(terminal),
            AppTool::Notes => crate::notes::run_notes_app(terminal),
            AppTool::OneOnOneManager => {
                crate::leadership_tools::run_tool(LeadershipTool::OneOnOne, terminal)
            }
            AppTool::DelegationTracker => {
                crate::leadership_tools::run_tool(LeadershipTool::Delegation, terminal)
            }
            AppTool::DecisionLog => {
                crate::leadership_tools::run_tool(LeadershipTool::Decision, terminal)
            }
        }
    }
}

#[derive(Clone, Default)]
struct TaskDashboard {
    db_path: PathBuf,
    topic_count: usize,
    task_count: usize,
    open_count: usize,
    done_count: usize,
    favourite_count: usize,
    recent_tasks: Vec<String>,
}

#[derive(Clone, Default)]
struct NotesDashboard {
    db_path: PathBuf,
    notes_root: PathBuf,
    db_note_count: usize,
    file_count: usize,
    directory_count: usize,
    recent_notes: Vec<String>,
    recent_files: Vec<String>,
}

#[derive(Clone, Default)]
struct HomepageDashboard {
    tasks: TaskDashboard,
    notes: NotesDashboard,
    one_on_ones: DashboardSnapshot,
    delegations: DashboardSnapshot,
    decisions: DashboardSnapshot,
    refreshed_at: String,
}

#[derive(Clone)]
struct FileScanSummary {
    file_count: usize,
    directory_count: usize,
    recent_files: Vec<String>,
}

/// Run the homepage (launcher) UI.
pub fn run_homepage(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    let tools = [
        AppTool::TaskManager,
        AppTool::Notes,
        AppTool::OneOnOneManager,
        AppTool::DelegationTracker,
        AppTool::DecisionLog,
    ];
    let mut selected = 0;
    let mut error_message: Option<String> = None;
    let mut dashboard = load_dashboard().unwrap_or_else(|err| {
        error_message = Some(err.to_string());
        HomepageDashboard {
            refreshed_at: "unavailable".to_string(),
            ..HomepageDashboard::default()
        }
    });
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
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

            draw_header(f, outer[0], &dashboard, error_message.as_deref());
            draw_dashboard(f, outer[1], &tools, selected, &dashboard);
            draw_footer(f, outer[2]);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') | KeyCode::Down => {
                        if selected < tools.len().saturating_sub(1) {
                            selected += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if selected > 0 {
                            selected -= 1;
                        }
                    }
                    KeyCode::Char('r') => match load_dashboard() {
                        Ok(new_dashboard) => {
                            dashboard = new_dashboard;
                            error_message = None;
                        }
                        Err(err) => error_message = Some(err.to_string()),
                    },
                    KeyCode::Enter => {
                        let mut tool = tools[selected];
                        error_message = tool.run(terminal).err().map(|err| err.to_string());
                        match load_dashboard() {
                            Ok(new_dashboard) => dashboard = new_dashboard,
                            Err(err) if error_message.is_none() => {
                                error_message = Some(err.to_string());
                            }
                            Err(_) => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
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
            draw_paths_panel(
                f,
                support[0],
                "Task Paths",
                &[
                    format!("DB: {}", compact_path(&dashboard.tasks.db_path, 22)),
                    format!("Topics: {}", dashboard.tasks.topic_count),
                    format!("Favourites: {}", dashboard.tasks.favourite_count),
                ],
            );
            draw_notes_snapshot(
                f,
                support[1],
                &[
                    format!(
                        "Notes: {} files / {} dirs",
                        dashboard.notes.file_count, dashboard.notes.directory_count
                    ),
                    format!("DB: {}", compact_path(&dashboard.notes.db_path, 22)),
                    format!("Root: {}", compact_path(&dashboard.notes.notes_root, 20)),
                ],
            );
        }
        AppTool::Notes => {
            draw_notes_snapshot(
                f,
                support[0],
                &[
                    format!("Notes DB: {}", compact_path(&dashboard.notes.db_path, 22)),
                    format!(
                        "Notes root: {}",
                        compact_path(&dashboard.notes.notes_root, 18)
                    ),
                    format!("DB notes: {}", dashboard.notes.db_note_count),
                ],
            );
            draw_paths_panel(
                f,
                support[1],
                "Task Context",
                &[
                    format!("Open tasks: {}", dashboard.tasks.open_count),
                    format!("Completed: {}", dashboard.tasks.done_count),
                    format!("Favourites: {}", dashboard.tasks.favourite_count),
                ],
            );
        }
        AppTool::OneOnOneManager => {
            draw_paths_panel(
                f,
                support[0],
                "1:1 Metrics",
                &[
                    format!(
                        "{}: {}",
                        dashboard.one_on_ones.stat_a_label, dashboard.one_on_ones.stat_a_value
                    ),
                    format!(
                        "{}: {}",
                        dashboard.one_on_ones.stat_b_label, dashboard.one_on_ones.stat_b_value
                    ),
                    format!(
                        "Store: {}",
                        compact_path(&dashboard.one_on_ones.store_path, 20)
                    ),
                ],
            );
            draw_notes_snapshot(
                f,
                support[1],
                &[
                    format!("Task backlog: {}", dashboard.tasks.open_count),
                    format!("Notes files: {}", dashboard.notes.file_count),
                    "Use for prep, follow-ups, and private reminders.".to_string(),
                ],
            );
        }
        AppTool::DelegationTracker => {
            draw_paths_panel(
                f,
                support[0],
                "Delegation Metrics",
                &[
                    format!(
                        "{}: {}",
                        dashboard.delegations.stat_a_label, dashboard.delegations.stat_a_value
                    ),
                    format!(
                        "{}: {}",
                        dashboard.delegations.stat_b_label, dashboard.delegations.stat_b_value
                    ),
                    format!(
                        "Store: {}",
                        compact_path(&dashboard.delegations.store_path, 20)
                    ),
                ],
            );
            draw_notes_snapshot(
                f,
                support[1],
                &[
                    format!("Task backlog: {}", dashboard.tasks.open_count),
                    format!("Recent notes: {}", dashboard.notes.recent_notes.len()),
                    "Use for follow-through and owner accountability.".to_string(),
                ],
            );
        }
        AppTool::DecisionLog => {
            draw_paths_panel(
                f,
                support[0],
                "Decision Metrics",
                &[
                    format!(
                        "{}: {}",
                        dashboard.decisions.stat_a_label, dashboard.decisions.stat_a_value
                    ),
                    format!(
                        "{}: {}",
                        dashboard.decisions.stat_b_label, dashboard.decisions.stat_b_value
                    ),
                    format!(
                        "Store: {}",
                        compact_path(&dashboard.decisions.store_path, 20)
                    ),
                ],
            );
            draw_notes_snapshot(
                f,
                support[1],
                &[
                    format!("Task backlog: {}", dashboard.tasks.open_count),
                    format!("Notes DB: {}", dashboard.notes.db_note_count),
                    "Use for rationale, impact, and later review.".to_string(),
                ],
            );
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
                    Span::styled("Follow-Ups: ", ui_style::muted_style()),
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
                    Span::styled("Blocked: ", ui_style::muted_style()),
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
                    Span::styled("Proposed: ", ui_style::muted_style()),
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

fn load_dashboard() -> Result<HomepageDashboard, Box<dyn Error>> {
    Ok(HomepageDashboard {
        tasks: load_task_dashboard()?,
        notes: load_notes_dashboard()?,
        one_on_ones: load_dashboard_snapshot(LeadershipTool::OneOnOne)?,
        delegations: load_dashboard_snapshot(LeadershipTool::Delegation)?,
        decisions: load_dashboard_snapshot(LeadershipTool::Decision)?,
        refreshed_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

fn load_task_dashboard() -> Result<TaskDashboard, Box<dyn Error>> {
    let db_path = crate::db::resolve_db_path(
        "TASK_MANAGER_DB_DIR",
        ".task_manager",
        "TASK_MANAGER_DB_FILENAME",
        "task_manager.db",
    );
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let database_url = format!("sqlite://{}", path_to_str(&db_path)?);
    let pool = crate::db::establish_connection_pool(&database_url)?;
    {
        let mut conn = pool.get()?;
        crate::db::run_migrations(&mut conn)?;
    }
    let mut conn = pool.get()?;

    let topics = topic::table
        .order_by(topic::id.asc())
        .load::<Topic>(&mut conn)?;
    let tasks = task::table
        .order_by(task::updated_at.desc())
        .load::<Task>(&mut conn)?;

    Ok(TaskDashboard {
        db_path,
        topic_count: topics.len(),
        task_count: tasks.len(),
        open_count: tasks.iter().filter(|task| !task.completed).count(),
        done_count: tasks.iter().filter(|task| task.completed).count(),
        favourite_count: tasks.iter().filter(|task| task.favourite).count(),
        recent_tasks: tasks
            .iter()
            .take(6)
            .map(|task| {
                format!(
                    "{} [{}] {}",
                    if task.completed { "done" } else { "open" },
                    task.updated_at,
                    compact_text(&task.name, 40)
                )
            })
            .collect(),
    })
}

fn load_notes_dashboard() -> Result<NotesDashboard, Box<dyn Error>> {
    let db_path =
        crate::db::resolve_db_path("NOTES_DB_DIR", ".notes", "NOTES_DB_FILENAME", "notes.db");
    let notes_root = std::env::var("NOTES_ROOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".notes/files"));
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&notes_root)?;

    let database_url = format!("sqlite://{}", path_to_str(&db_path)?);
    let pool = crate::db::establish_connection_pool(&database_url)?;
    {
        let mut conn = pool.get()?;
        crate::db::run_migrations(&mut conn)?;
    }
    let mut conn = pool.get()?;

    let notes = note::table
        .order_by(note::updated_at.desc())
        .load::<Note>(&mut conn)?;
    let file_scan = scan_notes_tree(&notes_root)?;

    Ok(NotesDashboard {
        db_path,
        notes_root,
        db_note_count: notes.len(),
        file_count: file_scan.file_count,
        directory_count: file_scan.directory_count,
        recent_notes: notes
            .iter()
            .take(6)
            .map(|note| format!("[{}] {}", note.updated_at, compact_text(&note.title, 42)))
            .collect(),
        recent_files: file_scan.recent_files,
    })
}

fn scan_notes_tree(root: &Path) -> Result<FileScanSummary, Box<dyn Error>> {
    if !root.exists() {
        return Ok(FileScanSummary {
            file_count: 0,
            directory_count: 0,
            recent_files: Vec::new(),
        });
    }

    let mut directory_count = 0usize;
    let mut file_entries = Vec::new();
    collect_note_files(root, root, &mut directory_count, &mut file_entries)?;
    file_entries.sort_by(|left, right| right.0.cmp(&left.0));

    Ok(FileScanSummary {
        file_count: file_entries.len(),
        directory_count,
        recent_files: file_entries
            .into_iter()
            .take(6)
            .map(|(_, path)| path)
            .collect(),
    })
}

fn collect_note_files(
    root: &Path,
    dir: &Path,
    directory_count: &mut usize,
    file_entries: &mut Vec<(SystemTime, String)>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            *directory_count += 1;
            collect_note_files(root, &path, directory_count, file_entries)?;
        } else {
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            file_entries.push((modified, compact_text(&relative, 48)));
        }
    }
    Ok(())
}

fn compact_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let char_count = trimmed.chars().count();
    if char_count <= max_chars {
        return trimmed.to_string();
    }

    trimmed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}

fn compact_path(path: &Path, max_chars: usize) -> String {
    compact_text(&path.display().to_string(), max_chars)
}

fn path_to_str(path: &Path) -> Result<&str, io::Error> {
    path.to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Path contains invalid Unicode"))
}

#[cfg(test)]
mod tests {
    use super::{compact_text, scan_notes_tree};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(format!("task_manager_cli_homepage_{unique}"))
    }

    #[test]
    fn compact_text_truncates_long_values() {
        assert_eq!(compact_text("alpha beta gamma", 8), "alpha b…");
        assert_eq!(compact_text("short", 8), "short");
    }

    #[test]
    fn scan_notes_tree_counts_directories_and_files() -> Result<(), Box<dyn std::error::Error>> {
        let root = temp_dir("scan_notes");
        fs::create_dir_all(root.join("projects/alpha"))?;
        fs::write(root.join("projects/alpha/roadmap.md"), b"# roadmap")?;
        fs::write(root.join("inbox.md"), b"# inbox")?;

        let summary = scan_notes_tree(&root)?;

        assert_eq!(summary.file_count, 2);
        assert_eq!(summary.directory_count, 2);
        assert_eq!(summary.recent_files.len(), 2);
        assert!(summary
            .recent_files
            .iter()
            .any(|path| path.contains("roadmap.md")));

        let _ = fs::remove_dir_all(root);
        Ok(())
    }
}
