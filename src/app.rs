use chrono::Local;
use rusqlite::{params, Connection, Result as SqlResult, ToSql};
use std::{cell::RefCell, collections::HashSet, error::Error, rc::Rc};

#[derive(Debug, Clone)]
pub struct Topic {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: i32,
    pub topic_id: i32,
    pub description: String,
    pub completed: bool,
    pub favourite: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// The mode of the application: either in normal navigation or adding a new task.
#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    AddingTask,
    EditingTask,
    AddingTopic,
    //EditingTopic,
}

/// The overall application state.
/// Manages state of the application
pub struct App {
    /// SQLite connection (wrapped in Rc/RefCell to share and mutate).
    pub conn: Rc<RefCell<Connection>>,
    /// Current list of Topics
    pub topics: Vec<Topic>,
    /// Current selected Topic
    pub selected_topic: usize,
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
        // Create topics table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS topic (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        // Create tasks table with a foreign key to topic
        conn.execute(
            "CREATE TABLE IF NOT EXISTS task (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic_id INTEGER NOT NULL,
                description TEXT NOT NULL,
                completed BOOLEAN NOT NULL DEFAULT 0,
                favourite BOOLEAN NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(topic_id) REFERENCES topic(id)
            )",
            [],
        )?;
        let mut app = App {
            conn: Rc::new(RefCell::new(conn)),
            topics: Vec::new(),
            tasks: Vec::new(),
            selected: 0,
            selected_topic: 0,
            input_mode: InputMode::Normal,
            input: String::new(),
            logs: Vec::new(),
            log_offset: 0,
            expanded: HashSet::new(),
        };
        app.load_topics()?;
        // Ensure Favourites topic exists.
        if !app.topics.iter().any(|t| t.name == "Favourites") {
            app.add_topic("Favourites")?;
            app.load_topics()?;
        }
        app.add_log("INFO", "Topics loaded");
        // Set default selected topic to Favourites
        if let Some((i, _)) = app
            .topics
            .iter()
            .enumerate()
            .find(|(_, t)| t.name == "Favourites")
        {
            app.selected_topic = i;
        }
        app.load_tasks()?;
        app.add_log("INFO", "Tasks loaded");
        app.add_log("INFO", "Application started");
        Ok(app)
    }

    /// Load tasks from the database.
    pub fn load_tasks(&mut self) -> SqlResult<()> {
        let conn = self.conn.borrow();
        self.tasks.clear();
        if self.topics.is_empty() {
            return Ok(());
        }
        let current_topic = &self.topics[self.selected_topic];
        let mut stmt = if current_topic.name == "Favourites" {
            conn.prepare("SELECT id, topic_id, description, completed, favourite, created_at, updated_at FROM task WHERE favourite = 1 ORDER BY id")?
        } else {
            conn.prepare("SELECT id, topic_id, description, completed, favourite, created_at, updated_at FROM task WHERE topic_id = ?1 ORDER BY id")?
        };

        let params: &[&dyn ToSql] = if current_topic.name == "Favourites" {
            &[]
        } else {
            // Note the reference to current_topic.id.
            &[&current_topic.id]
        };
        let task_iter = stmt.query_map(params, |row| {
            Ok(Task {
                id: row.get(0)?,
                topic_id: row.get(1)?,
                description: row.get(2)?,
                completed: row.get(3)?,
                favourite: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        for task in task_iter {
            self.tasks.push(task?);
        }
        if self.selected >= self.tasks.len() && !self.tasks.is_empty() {
            self.selected = self.tasks.len() - 1;
        }
        Ok(())
    }

    pub fn load_topics(&mut self) -> SqlResult<()> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, created_at, updated_at FROM topic ORDER BY id",
        )?;
        let topic_iter = stmt.query_map([], |row| {
            Ok(Topic {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2).unwrap_or_else(|_| "".to_string()),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        self.topics.clear();
        for topic in topic_iter {
            self.topics.push(topic?);
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
        let current_topic = &self.topics[self.selected_topic];
        if current_topic.name == "Favourites" {
            // Cannot add tasks directly to Favourites.
            return Ok(());
        }
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        {
            // Borrow the connection in its own scope.
            let conn = self.conn.borrow();
            conn.execute(
                "INSERT INTO task (topic_id, description, created_at, updated_at, completed) VALUES (?1, ?2, ?3, ?4, 0)",
                params![current_topic.id, desc, now, now],
            )?;
        } // conn borrow is dropped here.
        self.add_log("INFO", &format!("Added task: {}", desc));
        self.load_tasks()
    }

    pub fn add_topic<T: AsRef<str>>(&mut self, name: T) -> SqlResult<()> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let name_str = name.as_ref();
        {
            let conn = self.conn.borrow();
            conn.execute(
                "INSERT INTO topic (name, description, created_at, updated_at) VALUES (?1, '', ?2, ?3)",
                params![name_str, now, now],
            )?;
        }
        self.load_topics()?;
        Ok(())
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

    pub fn toggle_favourite(&mut self) -> SqlResult<()> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if let Some(task) = self.tasks.get(self.selected) {
            {
                let conn = self.conn.borrow();
                let new_fav = if task.favourite { 0 } else { 1 };
                conn.execute(
                    "UPDATE task SET favourite = ?1, updated_at = ?2 WHERE id = ?3",
                    params![new_fav, now, task.id],
                )?;
            }
            self.add_log(
                "INFO",
                &format!("Toggled favourite for task id: {}", task.id),
            );
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

    pub fn delete_topic(&mut self) -> SqlResult<()> {
        let current_topic = &self.topics[self.selected_topic];
        if current_topic.name == "Favourites" {
            // Do not delete Favourites topic.
            return Ok(());
        }
        {
            let conn = self.conn.borrow();
            conn.execute("DELETE FROM topic WHERE id = ?1", params![current_topic.id])?;
        }
        self.load_topics()?;
        self.selected_topic = 0;
        self.load_tasks()?;
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

    pub fn current_topic_is_favourites(&self) -> bool {
        if self.topics.is_empty() {
            false
        } else {
            self.topics[self.selected_topic].name == "Favourites"
        }
    }
}
