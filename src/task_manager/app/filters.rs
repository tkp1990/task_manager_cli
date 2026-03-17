use std::{error::Error, io};

use crate::db::task_manager::models::Task;
use crate::filter_presets::{save_presets, SavedFilterPreset};

use super::{App, InputMode};

impl App {
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

    pub(super) fn filter_tokens(query: &str) -> Vec<String> {
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
}
