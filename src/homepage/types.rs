use std::io::Stdout;
use std::path::PathBuf;

use crossterm::event::Event;
use tui::{backend::CrosstermBackend, style::Color, Terminal};

use crate::leadership_tools::{DashboardSnapshot, ToolKind as LeadershipTool};
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

    pub(crate) fn subtitle(&self) -> &'static str {
        match self {
            AppTool::TaskManager => "Tasks, topics, favourites.",
            AppTool::Notes => "Files, markdown, links.",
            AppTool::OneOnOneManager => "People, agenda, follow-ups.",
            AppTool::DelegationTracker => "Owners, status, due dates.",
            AppTool::DecisionLog => "Decisions, rationale, links.",
        }
    }

    pub(crate) fn accent(&self) -> Color {
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
    ) -> Result<(), Box<dyn std::error::Error>> {
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
pub struct TaskDashboard {
    pub db_path: PathBuf,
    pub topic_count: usize,
    pub task_count: usize,
    pub open_count: usize,
    pub done_count: usize,
    pub favourite_count: usize,
    pub recent_tasks: Vec<String>,
}

#[derive(Clone, Default)]
pub struct NotesDashboard {
    pub db_path: PathBuf,
    pub notes_root: PathBuf,
    pub db_note_count: usize,
    pub file_count: usize,
    pub directory_count: usize,
    pub recent_notes: Vec<String>,
    pub recent_files: Vec<String>,
}

#[derive(Clone, Default)]
pub struct HomepageDashboard {
    pub tasks: TaskDashboard,
    pub notes: NotesDashboard,
    pub one_on_ones: DashboardSnapshot,
    pub delegations: DashboardSnapshot,
    pub decisions: DashboardSnapshot,
    pub refreshed_at: String,
}

#[derive(Clone)]
pub struct FileScanSummary {
    pub file_count: usize,
    pub directory_count: usize,
    pub recent_files: Vec<String>,
}

pub enum HomepageAction {
    Continue,
    Exit,
    Refresh,
    Launch(AppTool),
}

pub fn handle_key(key: Event, selected: &mut usize, tool_count: usize) -> HomepageAction {
    let Event::Key(key) = key else {
        return HomepageAction::Continue;
    };

    match key.code {
        crossterm::event::KeyCode::Char('q') => HomepageAction::Exit,
        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
            if *selected < tool_count.saturating_sub(1) {
                *selected += 1;
            }
            HomepageAction::Continue
        }
        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
            if *selected > 0 {
                *selected -= 1;
            }
            HomepageAction::Continue
        }
        crossterm::event::KeyCode::Char('r') => HomepageAction::Refresh,
        crossterm::event::KeyCode::Enter => HomepageAction::Launch(match *selected {
            0 => AppTool::TaskManager,
            1 => AppTool::Notes,
            2 => AppTool::OneOnOneManager,
            3 => AppTool::DelegationTracker,
            _ => AppTool::DecisionLog,
        }),
        _ => HomepageAction::Continue,
    }
}
