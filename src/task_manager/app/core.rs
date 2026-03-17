use std::{collections::HashSet, error::Error, path::PathBuf};

use crate::db::task_manager::operations::DbOperations;
use crate::filter_presets::load_presets;

use super::{load_palette_history, App, InputMode};

impl App {
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
        let all_topics = app.db_ops.load_topics()?;

        if !all_topics.iter().any(|t| t.name == "Favourites") {
            app.add_topic("Favourites")?;
        }
        if !all_topics.iter().any(|t| t.name == "Default") {
            app.add_topic("Default")?;
        }
        if !all_topics.iter().any(|t| t.name == "Completed") {
            app.add_topic("Completed")?;
        }

        app.load_topics()?;
        app.add_log("INFO", "Topics loaded");

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

    pub fn load_tasks(&mut self) -> Result<(), Box<dyn Error>> {
        self.tasks.clear();
        if self.topics.is_empty() {
            self.selected = 0;
            return Ok(());
        }
        if self.selected_topic >= self.topics.len() {
            self.selected_topic = 0;
        }
        let current_topic = &self.topics[self.selected_topic];
        self.tasks = self.db_ops.load_tasks(current_topic)?;
        self.ensure_selected_visible();
        Ok(())
    }

    pub fn load_topics(&mut self) -> Result<(), Box<dyn Error>> {
        let all_topics = self.db_ops.load_topics()?;
        self.topics = all_topics
            .into_iter()
            .filter(|t| t.name != "Favourites" && t.name != "Completed")
            .collect();
        Ok(())
    }

    pub fn add_log(&mut self, level: &str, msg: &str) {
        crate::common::logs::push_timestamped_log(&mut self.logs, &mut self.log_offset, level, msg);
    }

    pub fn add_topic<T: AsRef<str>>(&mut self, name: T) -> Result<(), Box<dyn Error>> {
        let name_str = name.as_ref().trim();
        if name_str.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Topic name cannot be empty",
            )
            .into());
        }
        self.db_ops.add_topic(name_str, "")?;
        self.load_topics()?;
        Ok(())
    }

    pub fn focus_task_by_id(&mut self, task_id: i32) -> Result<bool, Box<dyn Error>> {
        let Some(task) = self.db_ops.find_task(task_id)? else {
            return Ok(false);
        };

        self.task_filter.clear();
        self.selected_topic = self
            .topics
            .iter()
            .position(|topic| topic.id == task.topic_id)
            .or_else(|| self.topics.iter().position(|topic| topic.name == "Default"))
            .unwrap_or(0);
        self.load_tasks()?;

        if let Some(index) = self
            .tasks
            .iter()
            .position(|candidate| candidate.id == task_id)
        {
            self.selected = index;
            self.ensure_selected_visible();
        }

        self.add_log("INFO", &format!("Focused task id: {}", task_id));
        Ok(true)
    }

    pub fn begin_command_palette(&mut self) {
        crate::common::palette::begin_palette(
            &mut self.command_palette_query,
            &mut self.command_palette_selected,
            &mut self.command_palette_return_mode,
            &mut self.input_mode,
            InputMode::CommandPalette,
        );
    }

    pub fn close_command_palette(&mut self) {
        crate::common::palette::close_palette(
            &mut self.command_palette_query,
            &mut self.command_palette_selected,
            self.command_palette_return_mode,
            &mut self.input_mode,
        );
    }

    pub fn record_palette_command(&mut self, command_id: &str) -> Result<(), Box<dyn Error>> {
        crate::common::palette::record_recent_command(
            &self.palette_history_store_path,
            &mut self.recent_palette_commands,
            command_id,
            8,
        )?;
        Ok(())
    }
}
