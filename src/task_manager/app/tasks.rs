use chrono::Local;
use std::{error::Error, io};

use crate::db::task_manager::models::TaskUpdate;

use super::{App, InputMode};

impl App {
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

    pub fn toggle_task(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(task) = self.tasks.get(self.selected) {
            self.db_ops.toggle_task_completion(task.id)?;
            self.add_log("INFO", &format!("Toggled task id: {}", task.id));
            self.load_tasks()?;
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
            self.load_special_tasks()?;
        }
        Ok(())
    }

    pub fn delete_task(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(task) = self.tasks.get(self.selected) {
            self.db_ops.delete_task(task.id)?;
            self.add_log("INFO", &format!("Deleted task id: {}", task.id));
            self.load_tasks()?;
            if self.selected > 0 && self.selected >= self.tasks.len() {
                self.selected -= 1;
            }
        }
        Ok(())
    }

    pub fn delete_topic(&mut self) -> Result<(), Box<dyn Error>> {
        let current_topic = &self.topics[self.selected_topic];
        if current_topic.name == "Favourites" {
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

    pub fn reset_task_inputs(&mut self) {
        self.task_name_input.clear();
        self.task_description_input.clear();
        self.task_form_message = None;
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
}
