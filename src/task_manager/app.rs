use chrono::Local;
use std::{collections::HashSet, error::Error};

use crate::db::task_manager::models::{Task, TaskUpdate, Topic};
use crate::db::task_manager::operations::DbOperations;

/// The mode of the application: either in normal navigation or adding a new task.
#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    AddingTask,
    AddingTaskName,
    AddingTaskDescription,
    EditingTask,
    DeleteTask,
    AddingTopic,
    Help,
    ViewingSpecialTopics,
    //EditingTopic,
}

/// The overall application state.
/// Manages state of the application
pub struct App {
    /// Database operations handler
    pub db_ops: DbOperations,
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
    /// Buffer for task name (when creating a new task)
    pub task_name_input: String,
    /// Buffer for task description (when creating a new task)
    pub task_description_input: String,
    /// Log storage.
    pub logs: Vec<String>,
    /// Scroll offset to be displayed.
    pub log_offset: usize,
    /// Set task IDs that are expanded
    pub expanded: HashSet<i32>,
    /// Flag for the help window
    pub show_help: bool,
    /// NEW for special topics popup
    pub special_tab_selected: usize, // 0 = Favourites, 1 = Completed
    pub special_task_selected: usize, // Selected task in special popup
    pub deleting_from_special: bool, // Track if delete is from special popup
    pub favourites_tasks: Vec<Task>,
    pub completed_tasks: Vec<Task>,
}

impl App {
    /// Create a new App instance. This opens the SQLite DB, creates the table if needed, loads tasks, and logs startup.
    pub fn new(db_path: &str) -> Result<App, Box<dyn Error>> {
        let db_path_string = format!("sqlite://{}", db_path);
        let pool = crate::db::establish_connection_pool(&db_path_string)?;

        // Run migrations if needed
        {
            let mut conn = pool.get()?;
            crate::db::run_migrations(&mut conn)?;
        }

        let db_ops = DbOperations::new(pool);
        let mut app = App {
            db_ops,
            topics: Vec::new(),
            tasks: Vec::new(),
            selected: 0,
            selected_topic: 0,
            input_mode: InputMode::Normal,
            input: String::new(),
            task_name_input: String::new(),
            task_description_input: String::new(),
            logs: Vec::new(),
            log_offset: 0,
            expanded: HashSet::new(),
            show_help: false,
            special_tab_selected: 0,
            special_task_selected: 0,
            deleting_from_special: false,
            favourites_tasks: Vec::new(),
            completed_tasks: Vec::new(),
        };
        // Load all topics (unfiltered) to check for special topics
        let all_topics = app.db_ops.load_topics()?;
        
        // Ensure Favourites topic exists.
        if !all_topics.iter().any(|t| t.name == "Favourites") {
            app.add_topic("Favourites")?;
        }
        // Ensure Default topic exists.
        if !all_topics.iter().any(|t| t.name == "Default") {
            app.add_topic("Default")?;
        }
        // Ensure Completed topic exists.
        if !all_topics.iter().any(|t| t.name == "Completed") {
            app.add_topic("Completed")?;
        }
        
        // Now load filtered topics for display
        app.load_topics()?;
        app.add_log("INFO", "Topics loaded");
        
        // Set default selected topic to Default (or first topic if Default doesn't exist)
        if let Some((i, _)) = app
            .topics
            .iter()
            .enumerate()
            .find(|(_, t)| t.name == "Default")
        {
            app.selected_topic = i;
        } else if !app.topics.is_empty() {
            app.selected_topic = 0;
        }
        app.load_tasks()?;
        app.add_log("INFO", "Tasks loaded");
        app.add_log("INFO", "Application started");
        Ok(app)
    }

    /// Load tasks from the database.
    pub fn load_tasks(&mut self) -> Result<(), Box<dyn Error>> {
        self.tasks.clear();
        if self.topics.is_empty() {
            self.selected = 0;
            return Ok(());
        }
        // Bounds check for selected_topic
        if self.selected_topic >= self.topics.len() {
            self.selected_topic = 0;
        }
        let current_topic = &self.topics[self.selected_topic];
        self.tasks = self.db_ops.load_tasks(current_topic)?;

        // Bounds check for selected task
        if self.selected >= self.tasks.len() && !self.tasks.is_empty() {
            self.selected = self.tasks.len() - 1;
        } else if self.tasks.is_empty() {
            self.selected = 0;
        }
        Ok(())
    }

    pub fn load_topics(&mut self) -> Result<(), Box<dyn Error>> {
        let all_topics = self.db_ops.load_topics()?;
        // Filter out Favourites and Completed from main topics
        self.topics = all_topics
            .into_iter()
            .filter(|t| t.name != "Favourites" && t.name != "Completed")
            .collect();
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

    /// Add a new task to the database with both name and description.
    pub fn add_task_with_details(&mut self, name: &str, desc: &str) -> Result<(), Box<dyn Error>> {
        let current_topic = &self.topics[self.selected_topic];
        if current_topic.name == "Favourites" {
            // Cannot add tasks directly to Favourites.
            return Ok(());
        }
        self.db_ops.add_task(current_topic.id, name, desc)?;
        self.add_log("INFO", &format!("Added task: {} - {}", name, desc));
        self.load_tasks()
    }

    /// Add a new task to the database.
    pub fn add_task(&mut self, desc: &str) -> Result<(), Box<dyn Error>> {
        let current_topic = &self.topics[self.selected_topic];
        if current_topic.name == "Favourites" {
            // Cannot add tasks directly to Favourites.
            return Ok(());
        }
        let name = if desc.len() > 20 {
            format!("Task {}", desc.chars().take(20).collect::<String>())
        } else {
            format!("Task {}", desc)
        };
        self.db_ops.add_task(current_topic.id, &name, desc)?;
        self.add_log("INFO", &format!("Added task: {}", desc));
        self.load_tasks()
    }

    pub fn add_topic<T: AsRef<str>>(&mut self, name: T) -> Result<(), Box<dyn Error>> {
        let name_str = name.as_ref();
        self.db_ops.add_topic(name_str, "")?;

        self.load_topics()?;
        Ok(())
    }

    /// Toggle the completion status of the currently selected task.
    pub fn toggle_task(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(task) = self.tasks.get(self.selected) {
            self.db_ops.toggle_task_completion(task.id)?;
            self.add_log("INFO", &format!("Toggled task id: {}", task.id));
            self.load_tasks()?;
            // Auto-refresh special tasks if popup might be open
            self.load_special_tasks()?;
        }
        Ok(())
    }

    pub fn toggle_favourite(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(task) = self.tasks.get(self.selected) {
            self.db_ops.toggle_task_favourite(task.id)?;
            self.add_log(
                "INFO",
                &format!("Toggled favourite for task id: {}", task.id),
            );
            self.load_tasks()?;
            // Auto-refresh special tasks if popup might be open
            self.load_special_tasks()?;
        }
        Ok(())
    }

    /// Delete the currently selected task.
    pub fn delete_task(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(task) = self.tasks.get(self.selected) {
            self.db_ops.delete_task(task.id)?;
            self.add_log("INFO", &format!("Deleted task id: {}", task.id));
            self.load_tasks()?;
            // Adjust selected index if needed.
            if self.selected > 0 && self.selected >= self.tasks.len() {
                self.selected -= 1;
            }
        }
        Ok(())
    }

    pub fn delete_topic(&mut self) -> Result<(), Box<dyn Error>> {
        let current_topic = &self.topics[self.selected_topic];
        if current_topic.name == "Favourites" {
            // Do not delete Favourites topic.
            return Ok(());
        }
        self.db_ops.delete_topic(current_topic.id)?;
        self.load_topics()?;
        self.selected_topic = 0;
        self.load_tasks()?;
        Ok(())
    }

    pub fn edit_task(&mut self, desc: &str) -> Result<(), Box<dyn Error>> {
        let t = self.tasks.get(self.selected);
        self.add_log("INFO", &format!("Task: {:?}", t));
        if let Some(task) = self.tasks.get(self.selected) {
            let update = TaskUpdate {
                name: Some(&task.name),
                description: Some(desc),
                completed: Some(task.completed),
                favourite: Some(task.favourite),
                updated_at: &Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            };
            self.db_ops.update_task(task.id, update)?;
            self.add_log(
                "INFO",
                &format!("Successfully edited task, with id: {}", task.id),
            );
            self.load_tasks()?;
        }
        Ok(())
    }

    pub fn current_topic_is_special(&self) -> bool {
        if self.topics.is_empty() {
            false
        } else {
            let name = &self.topics[self.selected_topic].name;
            name == "Favourites" || name == "Default"
        }
    }

    // Reset task input fields
    pub fn reset_task_inputs(&mut self) {
        self.task_name_input.clear();
        self.task_description_input.clear();
    }

    /// NEW: Load Favourites and Completed tasks
    pub fn load_special_tasks(&mut self) -> Result<(), Box<dyn Error>> {
        // Favourites
        let fav_topic = Topic {
            id: -1, // Not used
            name: "Favourites".to_string(),
            description: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        self.favourites_tasks = self.db_ops.load_tasks(&fav_topic)?;
        // Completed
        let completed_topic = Topic {
            id: -1,
            name: "Completed".to_string(),
            description: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        self.completed_tasks = self.db_ops.load_tasks(&completed_topic)?;
        
        // Bounds check for special_task_selected
        let current_tasks = if self.special_tab_selected == 0 {
            &self.favourites_tasks
        } else {
            &self.completed_tasks
        };
        if self.special_task_selected >= current_tasks.len() && !current_tasks.is_empty() {
            self.special_task_selected = current_tasks.len() - 1;
        } else if current_tasks.is_empty() {
            self.special_task_selected = 0;
        }
        Ok(())
    }

    /// Get current special tasks based on selected tab
    pub fn get_current_special_tasks(&self) -> &Vec<Task> {
        if self.special_tab_selected == 0 {
            &self.favourites_tasks
        } else {
            &self.completed_tasks
        }
    }

    /// Toggle completion for task in special popup
    pub fn toggle_special_task(&mut self) -> Result<(), Box<dyn Error>> {
        let tasks = self.get_current_special_tasks();
        if let Some(task) = tasks.get(self.special_task_selected) {
            self.db_ops.toggle_task_completion(task.id)?;
            self.add_log("INFO", &format!("Toggled task id: {}", task.id));
            // Reload both special task lists and main tasks
            self.load_special_tasks()?;
            self.load_tasks()?;
        }
        Ok(())
    }

    /// Toggle favourite for task in special popup
    pub fn toggle_special_favourite(&mut self) -> Result<(), Box<dyn Error>> {
        let tasks = self.get_current_special_tasks();
        if let Some(task) = tasks.get(self.special_task_selected) {
            self.db_ops.toggle_task_favourite(task.id)?;
            self.add_log("INFO", &format!("Toggled favourite for task id: {}", task.id));
            // Reload both special task lists and main tasks
            self.load_special_tasks()?;
            self.load_tasks()?;
        }
        Ok(())
    }

    /// Delete task in special popup
    pub fn delete_special_task(&mut self) -> Result<(), Box<dyn Error>> {
        let tasks = self.get_current_special_tasks();
        if let Some(task) = tasks.get(self.special_task_selected) {
            self.db_ops.delete_task(task.id)?;
            self.add_log("INFO", &format!("Deleted task id: {}", task.id));
            // Reload both special task lists and main tasks
            self.load_special_tasks()?;
            self.load_tasks()?;
            // Adjust selected index if needed
            let new_tasks = self.get_current_special_tasks();
            if self.special_task_selected > 0 && self.special_task_selected >= new_tasks.len() {
                self.special_task_selected -= 1;
            }
        }
        Ok(())
    }
}
