use chrono::Local;
use rusqlite::{params, Connection, Result as SqlResult};
use std::{cell::RefCell, collections::HashSet, error::Error, rc::Rc};

#[derive(Debug, Clone)]
pub struct Task {
    pub id: i32,
    pub description: String,
    pub completed: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// The mode of the application: either in normal navigation or adding a new task.
#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Adding,
    Editing,
}

/// The overall application state.
pub struct App {
    /// SQLite connection (wrapped in Rc/RefCell to share and mutate).
    pub conn: Rc<RefCell<Connection>>,
    /// Current list of tasks.
    pub tasks: Vec<Task>,
    /// Currently selected index in the task list.
    pub selected: usize,
    /// The current input mode.
    pub input_mode: InputMode,
    /// Buffer for new task input.
    pub input: String,
    /// Log storage.
    pub logs: Vec<String>,
    /// Scroll offset to be displayed.
    pub log_offset: usize,
    /// Set task IDs that are expanded
    pub expanded: HashSet<i32>,
}

impl App {
    /// Create a new App instance. This opens the SQLite DB, creates the table if needed, loads tasks, and logs startup.
    pub fn new(db_path: &str) -> Result<App, Box<dyn Error>> {
        let conn = Connection::open(db_path)?;
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
        let mut app = App {
            conn: Rc::new(RefCell::new(conn)),
            tasks: Vec::new(),
            selected: 0,
            input_mode: InputMode::Normal,
            input: String::new(),
            logs: Vec::new(),
            log_offset: 0,
            expanded: HashSet::new(),
        };
        app.load_tasks()?;
        app.add_log("INFO", "Application started");
        Ok(app)
    }

    /// Load tasks from the database.
    pub fn load_tasks(&mut self) -> SqlResult<()> {
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
    pub fn add_log(&mut self, level: &str, msg: &str) {
        let now = Local::now();
        let entry = format!("{} [{}] {}", now.format("%Y-%m-%d %H:%M:%S"), level, msg);
        self.logs.push(entry);
        // Reset scroll so that the latest logs are visible.
        self.log_offset = 0;
    }

    /// Add a new task to the database.
    pub fn add_task(&mut self, desc: &str) -> SqlResult<()> {
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
    pub fn toggle_task(&mut self) -> SqlResult<()> {
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
    pub fn delete_task(&mut self) -> SqlResult<()> {
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

    pub fn edit_task(&mut self, desc: &str) -> SqlResult<()> {
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
