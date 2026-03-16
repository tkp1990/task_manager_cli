use chrono::Local;
use std::{collections::HashSet, error::Error, io, path::PathBuf};

use crate::db::task_manager::models::{Task, TaskUpdate, Topic};
use crate::db::task_manager::operations::DbOperations;
use crate::filter_presets::{load_presets, save_presets, SavedFilterPreset};

/// The mode of the application: either in normal navigation or adding a new task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    CommandPalette,
    Filtering,
    FilteringSpecial,
    PresetFilters,
    PresetSpecialFilters,
    SavingPreset,
    SavingSpecialPreset,
    AddingTaskName,
    AddingTaskDescription,
    EditingTaskName,
    EditingTaskDescription,
    DeleteTask,
    DeleteSpecialTask,
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
    /// Current filter query for the visible task list.
    pub task_filter: String,
    /// Current selected preset in the presets popup.
    pub preset_selected: usize,
    /// User-defined presets persisted to disk.
    pub custom_task_presets: Vec<SavedFilterPreset>,
    /// Preset storage path.
    pub preset_store_path: PathBuf,
    /// Palette history storage path.
    pub palette_history_store_path: PathBuf,
    /// Currently selected index in the task list.
    pub selected: usize,
    /// The current input mode.
    pub input_mode: InputMode,
    /// The mode to return to after closing the command palette.
    pub command_palette_return_mode: InputMode,
    /// Palette query text.
    pub command_palette_query: String,
    /// Selected command in the palette popup.
    pub command_palette_selected: usize,
    /// Recently executed command ids, most recent first.
    pub recent_palette_commands: Vec<String>,
    /// Buffer for new task input.
    pub input: String,
    /// Buffer for task name (when creating a new task)
    pub task_name_input: String,
    /// Buffer for task description (when creating a new task)
    pub task_description_input: String,
    /// Inline feedback shown inside the task form popup.
    pub task_form_message: Option<String>,
    /// Buffer for naming a saved preset.
    pub preset_name_input: String,
    /// Inline feedback shown inside the preset popup.
    pub preset_form_message: Option<String>,
    /// Log storage.
    pub logs: Vec<String>,
    /// Scroll offset to be displayed.
    pub log_offset: usize,
    /// Set task IDs that are expanded
    pub expanded: HashSet<i32>,
    /// NEW for special topics popup
    pub special_tab_selected: usize, // 0 = Favourites, 1 = Completed
    pub special_task_selected: usize, // Selected task in special popup
    pub special_task_filter: String,
    pub favourites_tasks: Vec<Task>,
    pub completed_tasks: Vec<Task>,
}

impl App {
    /// Create a new App instance. This opens the SQLite DB, creates the table if needed, loads tasks, and logs startup.
    pub fn new(db_path: &str) -> Result<App, Box<dyn Error>> {
        let db_path_string = format!("sqlite://{}", db_path);
        let pool = crate::db::establish_connection_pool(&db_path_string)?;
        let preset_store_path = PathBuf::from(db_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("task_filter_presets.json");
        let palette_history_store_path = PathBuf::from(db_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("task_palette_history.json");

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
            task_filter: String::new(),
            preset_selected: 0,
            custom_task_presets: load_presets(&preset_store_path)?,
            preset_store_path,
            palette_history_store_path: palette_history_store_path.clone(),
            selected: 0,
            selected_topic: 0,
            input_mode: InputMode::Normal,
            command_palette_return_mode: InputMode::Normal,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            recent_palette_commands: load_palette_history(&palette_history_store_path)?,
            input: String::new(),
            task_name_input: String::new(),
            task_description_input: String::new(),
            task_form_message: None,
            preset_name_input: String::new(),
            preset_form_message: None,
            logs: Vec::new(),
            log_offset: 0,
            expanded: HashSet::new(),
            special_tab_selected: 0,
            special_task_selected: 0,
            special_task_filter: String::new(),
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
        self.ensure_selected_visible();
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
        let trimmed_name = name.trim();
        let trimmed_desc = desc.trim();
        if trimmed_name.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Task name cannot be empty").into(),
            );
        }

        let current_topic = &self.topics[self.selected_topic];
        if current_topic.name == "Favourites" {
            // Cannot add tasks directly to Favourites.
            return Ok(());
        }
        self.db_ops
            .add_task(current_topic.id, trimmed_name, trimmed_desc)?;
        self.add_log(
            "INFO",
            &format!("Added task: {} - {}", trimmed_name, trimmed_desc),
        );
        self.load_tasks()
    }

    pub fn add_topic<T: AsRef<str>>(&mut self, name: T) -> Result<(), Box<dyn Error>> {
        let name_str = name.as_ref().trim();
        if name_str.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Topic name cannot be empty").into(),
            );
        }
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

    pub fn edit_task(&mut self, name: &str, desc: &str) -> Result<(), Box<dyn Error>> {
        let trimmed_name = name.trim();
        let trimmed_desc = desc.trim();
        if trimmed_name.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Task name cannot be empty").into(),
            );
        }
        if let Some(task) = self.tasks.get(self.selected) {
            let update = TaskUpdate {
                name: Some(trimmed_name),
                description: Some(trimmed_desc),
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
        } else {
            self.add_log("WARN", "No task selected to edit");
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
        self.task_form_message = None;
    }

    pub fn has_task_filter(&self) -> bool {
        !self.task_filter.trim().is_empty()
    }

    pub fn filtered_task_indices(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| self.task_matches_filter(task, &self.task_filter))
            .map(|(index, _)| index)
            .collect()
    }

    fn topic_name_for_task(&self, topic_id: i32) -> Option<&str> {
        self.topics
            .iter()
            .find(|topic| topic.id == topic_id)
            .map(|topic| topic.name.as_str())
    }

    fn parse_bool_token(value: &str) -> Option<bool> {
        match value.trim().to_lowercase().as_str() {
            "true" | "yes" | "y" | "1" | "fav" | "favourite" | "favorite" | "star" | "starred" => {
                Some(true)
            }
            "false" | "no" | "n" | "0" | "none" | "plain" => Some(false),
            _ => None,
        }
    }

    fn parse_status_token(value: &str) -> Option<bool> {
        match value.trim().to_lowercase().as_str() {
            "done" | "complete" | "completed" | "closed" => Some(true),
            "open" | "todo" | "pending" | "active" => Some(false),
            _ => None,
        }
    }

    fn filter_tokens(query: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;

        for ch in query.chars() {
            match ch {
                '"' => in_quotes = !in_quotes,
                c if c.is_whitespace() && !in_quotes => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(ch),
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
    }

    fn task_matches_filter(&self, task: &Task, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }

        let name = task.name.to_lowercase();
        let description = task.description.to_lowercase();
        let topic_name = self
            .topic_name_for_task(task.topic_id)
            .unwrap_or_default()
            .to_lowercase();

        Self::filter_tokens(trimmed).into_iter().all(|token| {
            let (negated, token) = if let Some(token) = token.strip_prefix('-') {
                (true, token)
            } else {
                (false, token.as_str())
            };

            let matches = if let Some(value) = token.strip_prefix("topic:") {
                let value = value.to_lowercase();
                !value.is_empty() && topic_name.contains(&value)
            } else if let Some(value) = token
                .strip_prefix("status:")
                .or_else(|| token.strip_prefix("state:"))
            {
                Self::parse_status_token(value).is_some_and(|expected| task.completed == expected)
            } else if let Some(value) = token
                .strip_prefix("fav:")
                .or_else(|| token.strip_prefix("favorite:"))
                .or_else(|| token.strip_prefix("favourite:"))
                .or_else(|| token.strip_prefix("star:"))
            {
                Self::parse_bool_token(value).is_some_and(|expected| task.favourite == expected)
            } else {
                let token = token.to_lowercase();
                name.contains(&token) || description.contains(&token) || topic_name.contains(&token)
            };

            if negated {
                !matches
            } else {
                matches
            }
        })
    }

    pub fn ensure_selected_visible(&mut self) {
        let filtered = self.filtered_task_indices();
        if filtered.is_empty() {
            self.selected = 0;
            return;
        }

        if !filtered.contains(&self.selected) {
            self.selected = filtered[0];
        }
    }

    pub fn move_selection_down(&mut self) {
        let filtered = self.filtered_task_indices();
        if let Some(position) = filtered.iter().position(|index| *index == self.selected) {
            if position + 1 < filtered.len() {
                self.selected = filtered[position + 1];
            }
        } else if let Some(first) = filtered.first() {
            self.selected = *first;
        }
    }

    pub fn move_selection_up(&mut self) {
        let filtered = self.filtered_task_indices();
        if let Some(position) = filtered.iter().position(|index| *index == self.selected) {
            if position > 0 {
                self.selected = filtered[position - 1];
            }
        } else if let Some(first) = filtered.first() {
            self.selected = *first;
        }
    }

    pub fn begin_task_filter(&mut self) {
        self.input_mode = InputMode::Filtering;
    }

    pub fn begin_command_palette(&mut self) {
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.command_palette_return_mode = self.input_mode;
        self.input_mode = InputMode::CommandPalette;
    }

    pub fn close_command_palette(&mut self) {
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.input_mode = self.command_palette_return_mode;
    }

    pub fn record_palette_command(&mut self, command_id: &str) -> Result<(), Box<dyn Error>> {
        self.recent_palette_commands
            .retain(|item| item != command_id);
        self.recent_palette_commands
            .insert(0, command_id.to_string());
        self.recent_palette_commands.truncate(8);
        save_palette_history(
            &self.palette_history_store_path,
            &self.recent_palette_commands,
        )?;
        Ok(())
    }

    pub fn append_task_filter_char(&mut self, c: char) {
        self.task_filter.push(c);
        self.ensure_selected_visible();
    }

    pub fn pop_task_filter_char(&mut self) {
        self.task_filter.pop();
        self.ensure_selected_visible();
    }

    pub fn clear_task_filter(&mut self) {
        self.task_filter.clear();
        self.ensure_selected_visible();
    }

    pub fn task_filter_presets(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Open Tasks", "status:open"),
            ("Completed Tasks", "status:done"),
            ("Starred Tasks", "fav:true"),
            ("Unstarred Open", "status:open fav:false"),
            ("Work Focus", "topic:work status:open"),
        ]
    }

    pub fn all_task_filter_presets(&self) -> Vec<(String, String, bool)> {
        let mut presets: Vec<(String, String, bool)> = self
            .task_filter_presets()
            .into_iter()
            .map(|(name, query)| (name.to_string(), query.to_string(), true))
            .collect();
        presets.extend(
            self.custom_task_presets
                .iter()
                .cloned()
                .map(|preset| (preset.name, preset.query, false)),
        );
        presets
    }

    pub fn begin_task_presets(&mut self) {
        self.preset_selected = 0;
        self.input_mode = InputMode::PresetFilters;
    }

    pub fn begin_special_task_presets(&mut self) {
        self.preset_selected = 0;
        self.input_mode = InputMode::PresetSpecialFilters;
    }

    pub fn move_preset_down(&mut self, len: usize) {
        if self.preset_selected + 1 < len {
            self.preset_selected += 1;
        }
    }

    pub fn move_preset_up(&mut self) {
        if self.preset_selected > 0 {
            self.preset_selected -= 1;
        }
    }

    pub fn apply_selected_task_preset(&mut self) {
        if let Some((name, query, _)) = self
            .all_task_filter_presets()
            .get(self.preset_selected)
            .cloned()
        {
            self.task_filter = query;
            self.ensure_selected_visible();
            self.add_log("INFO", &format!("Applied preset: {}", name));
        }
    }

    pub fn apply_selected_special_task_preset(&mut self) {
        if let Some((name, query, _)) = self
            .all_task_filter_presets()
            .get(self.preset_selected)
            .cloned()
        {
            self.special_task_filter = query;
            self.ensure_special_selection_visible();
            self.add_log("INFO", &format!("Applied special preset: {}", name));
        }
    }

    pub fn clear_preset_form(&mut self) {
        self.preset_name_input.clear();
        self.preset_form_message = None;
    }

    pub fn begin_save_task_preset(&mut self) {
        if self.task_filter.trim().is_empty() {
            self.add_log("WARN", "Set a task filter before saving a preset");
            return;
        }
        self.clear_preset_form();
        self.input_mode = InputMode::SavingPreset;
    }

    pub fn begin_save_special_task_preset(&mut self) {
        if self.special_task_filter.trim().is_empty() {
            self.add_log("WARN", "Set a special task filter before saving a preset");
            return;
        }
        self.clear_preset_form();
        self.input_mode = InputMode::SavingSpecialPreset;
    }

    pub fn save_named_task_preset(&mut self, special: bool) -> Result<(), Box<dyn Error>> {
        let name = self.preset_name_input.trim();
        if name.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Preset name cannot be empty").into(),
            );
        }

        let query = if special {
            self.special_task_filter.trim()
        } else {
            self.task_filter.trim()
        };
        if query.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Filter query cannot be empty",
            )
            .into());
        }

        if let Some(existing) = self
            .custom_task_presets
            .iter_mut()
            .find(|preset| preset.name.eq_ignore_ascii_case(name))
        {
            existing.query = query.to_string();
        } else {
            self.custom_task_presets.push(SavedFilterPreset {
                name: name.to_string(),
                query: query.to_string(),
            });
        }
        save_presets(&self.preset_store_path, &self.custom_task_presets)?;
        self.add_log("INFO", &format!("Saved preset: {}", name));
        Ok(())
    }

    pub fn delete_selected_task_preset(&mut self) -> Result<bool, Box<dyn Error>> {
        let builtin_len = self.task_filter_presets().len();
        if self.preset_selected < builtin_len {
            return Ok(false);
        }

        let custom_index = self.preset_selected - builtin_len;
        if custom_index < self.custom_task_presets.len() {
            let removed = self.custom_task_presets.remove(custom_index);
            save_presets(&self.preset_store_path, &self.custom_task_presets)?;
            self.add_log("INFO", &format!("Deleted preset: {}", removed.name));
            if self.preset_selected > 0
                && self.preset_selected >= self.all_task_filter_presets().len()
            {
                self.preset_selected -= 1;
            }
            return Ok(true);
        }
        Ok(false)
    }

    pub fn clear_task_form_message(&mut self) {
        self.task_form_message = None;
    }

    pub fn set_task_form_message<T: Into<String>>(&mut self, message: T) {
        self.task_form_message = Some(message.into());
    }

    pub fn begin_add_task(&mut self) {
        if self.current_topic_is_special() {
            self.add_log("WARN", "Select a regular topic before adding a task");
            return;
        }
        self.reset_task_inputs();
        self.input_mode = InputMode::AddingTaskName;
    }

    pub fn cancel_add_task(&mut self) {
        self.reset_task_inputs();
        self.input_mode = InputMode::Normal;
    }

    pub fn begin_edit_task(&mut self) {
        if let Some(task) = self.tasks.get(self.selected) {
            self.task_name_input = task.name.clone();
            self.task_description_input = task.description.clone();
            self.input = task.description.clone();
            self.input_mode = InputMode::EditingTaskName;
        } else {
            self.add_log("WARN", "No task selected to edit");
        }
    }

    pub fn begin_delete_task(&mut self) {
        if self.tasks.is_empty() {
            self.add_log("WARN", "No task selected to delete");
            return;
        }
        self.input_mode = InputMode::DeleteTask;
    }

    pub fn begin_add_topic(&mut self) {
        self.input.clear();
        self.input_mode = InputMode::AddingTopic;
    }

    pub fn begin_delete_special_task(&mut self) {
        if self.get_current_special_tasks().is_empty() {
            self.add_log("WARN", "No task selected to delete");
            return;
        }
        self.input_mode = InputMode::DeleteSpecialTask;
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
        }
        self.ensure_special_selection_visible();
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

    pub fn has_special_task_filter(&self) -> bool {
        !self.special_task_filter.trim().is_empty()
    }

    pub fn filtered_special_task_indices(&self) -> Vec<usize> {
        self.get_current_special_tasks()
            .iter()
            .enumerate()
            .filter(|(_, task)| self.task_matches_filter(task, &self.special_task_filter))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn ensure_special_selection_visible(&mut self) {
        let filtered = self.filtered_special_task_indices();
        if filtered.is_empty() {
            self.special_task_selected = 0;
            return;
        }

        if !filtered.contains(&self.special_task_selected) {
            self.special_task_selected = filtered[0];
        }
    }

    pub fn move_special_selection_down(&mut self) {
        let filtered = self.filtered_special_task_indices();
        if let Some(position) = filtered
            .iter()
            .position(|index| *index == self.special_task_selected)
        {
            if position + 1 < filtered.len() {
                self.special_task_selected = filtered[position + 1];
            }
        } else if let Some(first) = filtered.first() {
            self.special_task_selected = *first;
        }
    }

    pub fn move_special_selection_up(&mut self) {
        let filtered = self.filtered_special_task_indices();
        if let Some(position) = filtered
            .iter()
            .position(|index| *index == self.special_task_selected)
        {
            if position > 0 {
                self.special_task_selected = filtered[position - 1];
            }
        } else if let Some(first) = filtered.first() {
            self.special_task_selected = *first;
        }
    }

    pub fn begin_special_task_filter(&mut self) {
        self.input_mode = InputMode::FilteringSpecial;
    }

    pub fn append_special_task_filter_char(&mut self, c: char) {
        self.special_task_filter.push(c);
        self.ensure_special_selection_visible();
    }

    pub fn pop_special_task_filter_char(&mut self) {
        self.special_task_filter.pop();
        self.ensure_special_selection_visible();
    }

    pub fn clear_special_task_filter(&mut self) {
        self.special_task_filter.clear();
        self.ensure_special_selection_visible();
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
            self.add_log(
                "INFO",
                &format!("Toggled favourite for task id: {}", task.id),
            );
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

fn load_palette_history(path: &std::path::Path) -> Result<Vec<String>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&content)?)
}

fn save_palette_history(path: &std::path::Path, commands: &[String]) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(commands)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{App, InputMode};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(format!("task_manager_cli_app_{unique}.db"))
    }

    #[test]
    fn begin_add_task_requires_regular_topic() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("special_topic");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.begin_add_task();

        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app
            .logs
            .last()
            .is_some_and(|entry| entry.contains("Select a regular topic before adding a task")));

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn begin_add_and_edit_task_manage_form_state() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("task_form");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_topic("Work")?;
        app.load_topics()?;
        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Work")
            .expect("work topic should exist");

        app.task_name_input = "stale".to_string();
        app.task_description_input = "stale".to_string();
        app.begin_add_task();
        assert_eq!(app.input_mode, InputMode::AddingTaskName);
        assert!(app.task_name_input.is_empty());
        assert!(app.task_description_input.is_empty());

        app.add_task_with_details("Write docs", "Document the app flow")?;
        app.begin_edit_task();
        assert_eq!(app.input_mode, InputMode::EditingTaskName);
        assert_eq!(app.task_name_input, "Write docs");
        assert_eq!(app.task_description_input, "Document the app flow");

        app.edit_task("Updated docs", "Updated description")?;
        assert_eq!(app.tasks[0].name, "Updated docs");
        assert_eq!(app.tasks[0].description, "Updated description");

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn reset_task_inputs_clears_inline_feedback() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("task_feedback");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.task_name_input = "Draft".to_string();
        app.task_description_input = "Draft description".to_string();
        app.set_task_form_message("Task name cannot be empty");

        app.reset_task_inputs();

        assert!(app.task_name_input.is_empty());
        assert!(app.task_description_input.is_empty());
        assert!(app.task_form_message.is_none());

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn task_filter_repositions_selection_to_visible_result(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("task_filter");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_topic("Work")?;
        app.load_topics()?;
        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Work")
            .expect("work topic should exist");

        app.add_task_with_details("Alpha", "First task")?;
        app.add_task_with_details("Beta", "Second task")?;

        app.selected = 1;
        app.task_filter = "Alpha".to_string();
        app.ensure_selected_visible();

        assert_eq!(app.filtered_task_indices(), vec![0]);
        assert_eq!(app.selected, 0);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn special_task_filter_repositions_selection_to_visible_result(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("special_task_filter");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_topic("Work")?;
        app.load_topics()?;
        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Work")
            .expect("work topic should exist");

        app.add_task_with_details("Alpha", "First task")?;
        app.add_task_with_details("Beta", "Second task")?;

        app.selected = 1;
        app.toggle_favourite()?;
        app.selected = 0;
        app.toggle_favourite()?;
        app.load_special_tasks()?;

        app.special_task_selected = 1;
        app.special_task_filter = "Alpha".to_string();
        app.ensure_special_selection_visible();

        assert_eq!(app.filtered_special_task_indices(), vec![0]);
        assert_eq!(app.special_task_selected, 0);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn task_filter_supports_status_topic_and_favourite_tokens(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("task_filter_tokens");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_topic("Work")?;
        app.add_topic("Personal")?;
        app.load_topics()?;
        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Work")
            .expect("work topic should exist");
        app.add_task_with_details("Alpha", "Urgent work item")?;

        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Personal")
            .expect("personal topic should exist");
        app.add_task_with_details("Beta", "Personal errand")?;

        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Default")
            .expect("default topic should exist");
        app.load_tasks()?;
        app.selected = 1;
        app.toggle_task()?;
        app.toggle_favourite()?;

        app.task_filter = "status:done topic:personal fav:true".to_string();
        assert_eq!(app.filtered_task_indices(), vec![1]);

        app.task_filter = "status:open topic:work".to_string();
        assert_eq!(app.filtered_task_indices(), vec![0]);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn task_filter_supports_phrases_and_negation() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("task_filter_phrases");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_topic("Work Projects")?;
        app.load_topics()?;
        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Work Projects")
            .expect("work projects topic should exist");

        app.add_task_with_details("Project Alpha", "Review quarterly roadmap")?;
        app.add_task_with_details("Project Beta", "Prepare launch checklist")?;

        app.task_filter = "\"Project Alpha\"".to_string();
        assert_eq!(app.filtered_task_indices(), vec![0]);

        app.task_filter = "topic:\"Work Projects\" -beta".to_string();
        assert_eq!(app.filtered_task_indices(), vec![0]);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn command_palette_round_trips_mode_and_query() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("command_palette");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.begin_task_filter();
        app.command_palette_query = "stale".to_string();
        app.begin_command_palette();

        assert_eq!(app.input_mode, InputMode::CommandPalette);
        assert_eq!(app.command_palette_return_mode, InputMode::Filtering);
        assert!(app.command_palette_query.is_empty());

        app.close_command_palette();
        assert_eq!(app.input_mode, InputMode::Filtering);

        app.record_palette_command("add_task")?;
        app.record_palette_command("help")?;
        app.record_palette_command("add_task")?;
        assert_eq!(app.recent_palette_commands[0], "add_task");
        assert_eq!(app.recent_palette_commands[1], "help");

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn applying_task_preset_sets_filter_and_selection() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("task_preset");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_topic("Work")?;
        app.load_topics()?;
        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Work")
            .expect("work topic should exist");
        app.add_task_with_details("Alpha", "Open task")?;
        app.add_task_with_details("Beta", "Done task")?;
        app.selected = 1;
        app.toggle_task()?;

        app.preset_selected = 0;
        app.apply_selected_task_preset();

        assert_eq!(app.task_filter, "status:open");
        assert_eq!(app.filtered_task_indices(), vec![0]);
        assert_eq!(app.selected, 0);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn applying_special_task_preset_sets_special_filter() -> Result<(), Box<dyn std::error::Error>>
    {
        let db_path = temp_db_path("special_task_preset");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_topic("Work")?;
        app.load_topics()?;
        app.selected_topic = app
            .topics
            .iter()
            .position(|topic| topic.name == "Work")
            .expect("work topic should exist");
        app.add_task_with_details("Alpha", "First task")?;
        app.add_task_with_details("Beta", "Second task")?;

        app.selected = 0;
        app.toggle_favourite()?;
        app.selected = 1;
        app.toggle_favourite()?;
        app.load_special_tasks()?;

        app.preset_selected = 2;
        app.apply_selected_special_task_preset();

        assert_eq!(app.special_task_filter, "fav:true");
        assert_eq!(app.filtered_special_task_indices(), vec![0, 1]);

        let _ = fs::remove_file(db_path);
        Ok(())
    }
}
