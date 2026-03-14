use crossterm::event::{self, Event, KeyCode};
use std::io::Stdout;
use std::{
    error::Error,
    time::{Duration, Instant},
};
use tui::backend::CrosstermBackend;
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

/// Enum representing a tool available from the homepage.
#[derive(Clone)]
pub enum AppTool {
    TaskManager,
    Notes,
    // Future tools can be added here.
}

impl AppTool {
    pub fn title(&self) -> &'static str {
        match self {
            AppTool::TaskManager => "Task Manager",
            AppTool::Notes => "Notes",
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            AppTool::TaskManager => {
                // Launch the Task Manager tool.
                crate::task_manager::run_task_manager(terminal)
            }
            AppTool::Notes => {
                // Launch the Notes app.
                crate::notes::run_notes_app(terminal)
            }
        }
    }
}

/// Run the homepage (launcher) UI.
pub fn run_homepage(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    let tools = vec![AppTool::TaskManager, AppTool::Notes];
    let mut selected = 0;
    let mut error_message: Option<String> = None;
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Length(3), Constraint::Min(3)].as_ref())
                .split(size);

            let header_text = match &error_message {
                Some(message) => format!(
                    "Homepage - Select a Tool (Use arrow keys and Enter). Press q to quit. Last error: {message}"
                ),
                None => "Homepage - Select a Tool (Use arrow keys and Enter). Press q to quit."
                    .to_string(),
            };
            let header =
                Paragraph::new(header_text).block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let items: Vec<ListItem> = tools
                .iter()
                .map(|tool| ListItem::new(Span::raw(tool.title())))
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(selected));
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Available Tools"),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                );
            f.render_stateful_widget(list, chunks[1], &mut list_state);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down => {
                        if selected < tools.len().saturating_sub(1) {
                            selected += 1;
                        }
                    }
                    KeyCode::Up => {
                        if selected > 0 {
                            selected -= 1;
                        }
                    }
                    KeyCode::Enter => {
                        let mut tool = tools[selected].clone();
                        error_message = tool.run(terminal).err().map(|err| err.to_string());
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
