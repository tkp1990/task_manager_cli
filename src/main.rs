use chrono::Local;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rusqlite::{params, Connection, Result as SqlResult};
use std::{
    cell::RefCell,
    collections::HashSet,
    error::Error,
    io,
    rc::Rc,
    time::{Duration, Instant},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

/// Our Task struct.
#[derive(Debug, Clone)]
struct Task {
    id: i32,
    description: String,
    completed: bool,
    created_at: String,
    updated_at: String,
}

/// The mode of the application: either in normal navigation or adding a new task.
#[derive(PartialEq)]
enum InputMode {
    Normal,
    Adding,
    Editing,
}

/// The overall application state.
struct App {
    /// SQLite connection (wrapped in Rc/RefCell to share and mutate).
    conn: Rc<RefCell<Connection>>,
    /// Current list of tasks.
    tasks: Vec<Task>,
    /// Currently selected index in the task list.
    selected: usize,
    /// The current input mode.
    input_mode: InputMode,
    /// Buffer for new task input.
    input: String,
    /// Log storage.
    logs: Vec<String>,
    /// Scroll offset to be displayed.
    log_offset: usize,
    /// Set task IDs that are expanded
    expanded: HashSet<i32>,
}

impl App {
    /// Load tasks from the database.
    fn load_tasks(&mut self) -> SqlResult<()> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, description, completed, created_at, updated_at FROM task ORDER BY id",
        )?;
        let task_iter = stmt.query_map([], |row| {
            Ok(Task {
                id: row.get(0)?,
                description: row.get(1)?,
                completed: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        self.tasks.clear();
        for task in task_iter {
            self.tasks.push(task?);
        }
        // Reset selection if needed.
        if self.selected >= self.tasks.len() && !self.tasks.is_empty() {
            self.selected = self.tasks.len() - 1;
        }
        Ok(())
    }

    /// Add logs to the local instance of app
    fn add_log(&mut self, level: &str, msg: &str) {
        let now = Local::now();
        let entry = format!("{} [{}] {}", now.format("%Y-%m-%d %H:%M:%S"), level, msg);
        self.logs.push(entry);
        // Reset scroll so that the latest logs are visible.
        self.log_offset = 0;
    }

    /// Add a new task to the database.
    fn add_task(&mut self, desc: &str) -> SqlResult<()> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        {
            // Borrow the connection in its own scope.
            let conn = self.conn.borrow();
            conn.execute(
                "INSERT INTO task (description, created_at, updated_at, completed) VALUES (?1, ?2, ?3, 0)",
                params![desc, now, now],
            )?;
        } // conn borrow is dropped here.
        self.add_log("INFO", &format!("Added task: {}", desc));
        self.load_tasks()
    }

    /// Toggle the completion status of the currently selected task.
    fn toggle_task(&mut self) -> SqlResult<()> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if let Some(task) = self.tasks.get(self.selected) {
            {
                let conn = self.conn.borrow();
                let new_status = if task.completed { 0 } else { 1 };
                conn.execute(
                    "UPDATE task SET completed = ?1, updated_at = ?2 WHERE id = ?3",
                    params![new_status, now, task.id],
                )?;
            }
            self.add_log("INFO", &format!("Toggled task id: {}", task.id));
            self.load_tasks()?;
        }
        Ok(())
    }

    /// Delete the currently selected task.
    fn delete_task(&mut self) -> SqlResult<()> {
        if let Some(task) = self.tasks.get(self.selected) {
            {
                let conn = self.conn.borrow();
                conn.execute("DELETE FROM task WHERE id = ?1", params![task.id])?;
            }
            self.add_log("INFO", &format!("Deleted task id: {}", task.id));
            self.load_tasks()?;
            // Adjust selected index if needed.
            if self.selected > 0 && self.selected >= self.tasks.len() {
                self.selected -= 1;
            }
        }
        Ok(())
    }

    fn edit_task(&mut self, desc: &str) -> SqlResult<()> {
        let t = self.tasks.get(self.selected);
        self.add_log("INFO", &format!("Task: {:?}", t));
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if let Some(task) = self.tasks.get(self.selected) {
            {
                let conn = self.conn.borrow();
                conn.execute(
                    "UPDATE task SET description = ?1, updated_at = ?2 where id = ?3",
                    params![desc, now, task.id],
                )?;
            }
            self.add_log(
                "INFO",
                &format!("Successfully edited task, with id: {}", task.id),
            );
            self.load_tasks()?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // --- SETUP THE DATABASE ---
    let conn = Connection::open("task_manager.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            description TEXT NOT NULL,
            completed BOOLEAN NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;
    let db = Rc::new(RefCell::new(conn));

    // --- INITIALIZE APPLICATION STATE ---
    let mut app = App {
        conn: db,
        tasks: Vec::new(),
        selected: 0,
        input_mode: InputMode::Normal,
        input: String::new(),
        logs: Vec::new(),
        log_offset: 0,
        expanded: HashSet::new(),
    };
    app.load_tasks()?; // load initial tasks

    // --- SETUP TERMINAL ---
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // --- EVENT LOOP ---
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| {
            // Define the layout.
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Min(3),
                        Constraint::Length(3),
                        Constraint::Length(3),
                        Constraint::Length(15),
                    ]
                    .as_ref(),
                )
                .split(size);

            // --- TASKS SECTION ---
            let items: Vec<ListItem> = app
                .tasks
                .iter()
                .map(|task| {
                    // If task is expanded, show extra details; otherwise, only show the task name.
                    let lines = if app.expanded.contains(&task.id) {
                        vec![
                            Spans::from(Span::raw(format!("{}", task.description))),
                            Spans::from(Span::styled(
                                format!(
                                    "ID: {} | Completed: {} | Created: {} | Updated: {}",
                                    task.id,
                                    if task.completed { "Yes" } else { "No" },
                                    task.created_at,
                                    task.updated_at
                                ),
                                Style::default().fg(Color::Gray),
                            )),
                        ]
                    } else {
                        vec![Spans::from(Span::raw(format!("{}", task.description)))]
                    };
                    ListItem::new(lines)
                })
                .collect();

            let tasks_list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Tasks"))
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            let mut list_state = ListState::default();
            list_state.select(Some(app.selected));
            f.render_stateful_widget(tasks_list, chunks[0], &mut list_state);

            // Instructions or input area.
            let (msg, style) = match app.input_mode {
                InputMode::Normal => (
                    vec![
                        Span::raw("Press "),
                        Span::styled("a", Style::default().fg(Color::Yellow)),
                        Span::raw(" to add, "),
                        Span::styled("t", Style::default().fg(Color::Yellow)),
                        Span::raw(" to toggle, "),
                        Span::styled("e", Style::default().fg(Color::Yellow)),
                        Span::raw(" to edit selected task, "),
                        Span::styled("d", Style::default().fg(Color::Yellow)),
                        Span::raw(" to delete, Up/Down or j/k to navigate, "),
                        Span::styled("d", Style::default().fg(Color::Yellow)),
                        Span::raw(
                            " to delete, Up/Down to navigate, PageUp/PageDown to scroll logs, ",
                        ),
                        Span::styled("q", Style::default().fg(Color::Yellow)),
                        Span::raw(" to quit."),
                    ],
                    Style::default(),
                ),
                InputMode::Adding => (
                    vec![
                        Span::raw("Enter task description (Press Enter to add, Esc to cancel): "),
                        Span::raw(&app.input),
                    ],
                    Style::default().fg(Color::Green),
                ),
                InputMode::Editing => (
                    vec![
                        Span::raw("Edit task description (Press Enter to save, Esc to cancel):"),
                        Span::raw(&app.input),
                    ],
                    Style::default().fg(Color::Green),
                ),
            };

            let help_message = Paragraph::new(Spans::from(msg))
                .style(style)
                .block(Block::default().borders(Borders::ALL).title("Instructions"));

            f.render_widget(help_message, chunks[1]);

            // (Optional) Show current mode at the bottom.
            let mode_text = match app.input_mode {
                InputMode::Normal => "Normal Mode",
                InputMode::Adding => "Add Mode",
                InputMode::Editing => "Editing Mode",
            };
            let mode = Paragraph::new(mode_text)
                .block(Block::default().borders(Borders::ALL).title("Mode"));
            f.render_widget(mode, chunks[2]);

            // --- LOGS SECTION ---
            // Determine how many lines can be shown.
            let log_area_height = chunks[3].height as usize;
            let total_logs = app.logs.len();
            // Compute the starting index, ensuring we don't underflow.
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
        })?;

        // Poll for input or tick.
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('a') => {
                            app.input_mode = InputMode::Adding;
                            app.input.clear();
                        }
                        KeyCode::Char('e') => {
                            app.input_mode = InputMode::Editing;
                            app.input.clear();
                        }
                        KeyCode::Char('t') => {
                            if let Err(e) = app.toggle_task() {
                                eprintln!("Error toggling task: {:?}", e);
                            }
                        }
                        KeyCode::Char('d') => {
                            if let Err(e) = app.delete_task() {
                                eprintln!("Error deleting task: {:?}", e);
                            }
                        }
                        KeyCode::Enter => {
                            // Toggle expansion of the selected task.
                            if let Some(task) = app.tasks.get(app.selected) {
                                if app.expanded.contains(&task.id) {
                                    app.expanded.remove(&task.id);
                                } else {
                                    app.expanded.insert(task.id);
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if app.selected < app.tasks.len().saturating_sub(1) {
                                app.selected += 1;
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if app.selected > 0 {
                                app.selected -= 1;
                            }
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
                    InputMode::Adding => match key.code {
                        KeyCode::Enter => {
                            // Add the task and switch back to normal mode.
                            if !app.input.is_empty() {
                                let input_clone = app.input.clone();
                                if let Err(e) = app.add_task(&input_clone) {
                                    eprintln!("Error adding task: {:?}", e);
                                }
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            // Add the task and switch back to normal mode.
                            if !app.input.is_empty() {
                                let input_clone = app.input.clone();
                                if let Err(e) = app.edit_task(&input_clone) {
                                    eprintln!("Error editing task: {:?}", e);
                                }
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
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

    // --- CLEANUP ---
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
