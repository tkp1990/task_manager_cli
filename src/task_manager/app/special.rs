use std::error::Error;

use crate::db::task_manager::models::{Task, Topic};

use super::{App, InputMode};

impl App {
    pub fn begin_delete_special_task(&mut self) {
        if self.get_current_special_tasks().is_empty() {
            self.add_log("WARN", "No task selected to delete");
            return;
        }
        self.input_mode = InputMode::DeleteSpecialTask;
    }

    pub fn load_special_tasks(&mut self) -> Result<(), Box<dyn Error>> {
        let fav_topic = Topic {
            id: -1,
            name: "Favourites".to_string(),
            description: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        self.favourites_tasks = self.db_ops.load_tasks(&fav_topic)?;

        let completed_topic = Topic {
            id: -1,
            name: "Completed".to_string(),
            description: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        self.completed_tasks = self.db_ops.load_tasks(&completed_topic)?;

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

    pub fn get_current_special_tasks(&self) -> &Vec<Task> {
        if self.special_tab_selected == 0 {
            &self.favourites_tasks
        } else {
            &self.completed_tasks
        }
    }

    pub fn toggle_special_task(&mut self) -> Result<(), Box<dyn Error>> {
        let tasks = self.get_current_special_tasks();
        if let Some(task) = tasks.get(self.special_task_selected) {
            self.db_ops.toggle_task_completion(task.id)?;
            self.add_log("INFO", &format!("Toggled task id: {}", task.id));
            self.load_special_tasks()?;
            self.load_tasks()?;
        }
        Ok(())
    }

    pub fn toggle_special_favourite(&mut self) -> Result<(), Box<dyn Error>> {
        let tasks = self.get_current_special_tasks();
        if let Some(task) = tasks.get(self.special_task_selected) {
            self.db_ops.toggle_task_favourite(task.id)?;
            self.add_log(
                "INFO",
                &format!("Toggled favourite for task id: {}", task.id),
            );
            self.load_special_tasks()?;
            self.load_tasks()?;
        }
        Ok(())
    }

    pub fn delete_special_task(&mut self) -> Result<(), Box<dyn Error>> {
        let tasks = self.get_current_special_tasks();
        if let Some(task) = tasks.get(self.special_task_selected) {
            self.db_ops.delete_task(task.id)?;
            self.add_log("INFO", &format!("Deleted task id: {}", task.id));
            self.load_special_tasks()?;
            self.load_tasks()?;
            let new_tasks = self.get_current_special_tasks();
            if self.special_task_selected > 0 && self.special_task_selected >= new_tasks.len() {
                self.special_task_selected -= 1;
            }
        }
        Ok(())
    }
}
