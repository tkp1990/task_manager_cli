use chrono::Local;
use std::{error::Error, io, time::Instant};

use crate::db::notes::models::{Note, NoteUpdate};

use super::{App, InputMode, AUTOSAVE_IDLE_DELAY};

impl App {
    pub fn add_note(&mut self, title: &str, content: &str) -> Result<(), Box<dyn Error>> {
        let trimmed_title = title.trim();
        if trimmed_title.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Note title cannot be empty").into(),
            );
        }
        self.db_ops.add_note(trimmed_title, content)?;
        self.add_log("INFO", &format!("Added note: {}", trimmed_title));
        self.load_notes()
    }

    pub fn update_note(
        &mut self,
        note_id: i32,
        title: &str,
        content: &str,
    ) -> Result<(), Box<dyn Error>> {
        let trimmed_title = title.trim();
        if trimmed_title.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Note title cannot be empty").into(),
            );
        }
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let update = NoteUpdate {
            title: Some(trimmed_title),
            content: Some(content),
            updated_at: &now,
        };
        self.db_ops.update_note(note_id, update)?;
        self.add_log("INFO", &format!("Updated note id: {}", note_id));
        self.load_notes()
    }

    pub fn delete_note(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(note) = self.notes.get(self.selected) {
            self.db_ops.delete_note(note.id)?;
            self.add_log("INFO", &format!("Deleted note id: {}", note.id));
            self.load_notes()?;
            if self.selected > 0 && self.selected >= self.notes.len() {
                self.selected -= 1;
            }
        }
        Ok(())
    }

    pub fn reset_inputs(&mut self) {
        self.title_input.clear();
        self.content_input.clear();
        self.note_form_message = None;
        self.note_form_dirty = false;
        self.note_form_last_change_at = None;
        self.editing_title = true;
    }

    pub fn has_note_filter(&self) -> bool {
        !self.note_filter.trim().is_empty()
    }

    pub fn filtered_note_indices(&self) -> Vec<usize> {
        self.notes
            .iter()
            .enumerate()
            .filter(|(_, note)| self.note_matches_filter(note, &self.note_filter))
            .map(|(index, _)| index)
            .collect()
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

    fn note_matches_filter(&self, note: &Note, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }

        let title = note.title.to_lowercase();
        let content = note.content.to_lowercase();

        Self::filter_tokens(trimmed).into_iter().all(|token| {
            let (negated, token) = if let Some(token) = token.strip_prefix('-') {
                (true, token)
            } else {
                (false, token.as_str())
            };

            let matches = if let Some(value) = token
                .strip_prefix("title:")
                .or_else(|| token.strip_prefix("name:"))
            {
                let value = value.to_lowercase();
                !value.is_empty() && title.contains(&value)
            } else if let Some(value) = token
                .strip_prefix("body:")
                .or_else(|| token.strip_prefix("content:"))
            {
                let value = value.to_lowercase();
                !value.is_empty() && content.contains(&value)
            } else {
                let token = token.to_lowercase();
                title.contains(&token) || content.contains(&token)
            };

            if negated {
                !matches
            } else {
                matches
            }
        })
    }

    pub fn ensure_selected_visible(&mut self) {
        let filtered = self.filtered_note_indices();
        if filtered.is_empty() {
            self.selected = 0;
            return;
        }

        if !filtered.contains(&self.selected) {
            self.selected = filtered[0];
        }
    }

    pub fn move_selection_down(&mut self) {
        let filtered = self.filtered_note_indices();
        if let Some(position) = filtered.iter().position(|index| *index == self.selected) {
            if position + 1 < filtered.len() {
                self.selected = filtered[position + 1];
            }
        } else if let Some(first) = filtered.first() {
            self.selected = *first;
        }
    }

    pub fn move_selection_up(&mut self) {
        let filtered = self.filtered_note_indices();
        if let Some(position) = filtered.iter().position(|index| *index == self.selected) {
            if position > 0 {
                self.selected = filtered[position - 1];
            }
        } else if let Some(first) = filtered.first() {
            self.selected = *first;
        }
    }

    pub fn begin_note_filter(&mut self) {
        self.input_mode = InputMode::Filtering;
    }

    pub fn append_note_filter_char(&mut self, c: char) {
        self.note_filter.push(c);
        self.ensure_selected_visible();
    }

    pub fn pop_note_filter_char(&mut self) {
        self.note_filter.pop();
        self.ensure_selected_visible();
    }

    pub fn clear_note_filter(&mut self) {
        self.note_filter.clear();
        self.ensure_selected_visible();
    }

    pub fn note_filter_presets(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Project Notes", "title:project"),
            ("Meeting Notes", "body:meeting"),
            ("Shopping Lists", "title:shopping"),
            ("Roadmap Notes", "\"roadmap\""),
            ("Exclude Shopping", "-title:shopping"),
        ]
    }

    pub fn all_note_filter_presets(&self) -> Vec<(String, String, bool)> {
        let mut presets: Vec<(String, String, bool)> = self
            .note_filter_presets()
            .into_iter()
            .map(|(name, query)| (name.to_string(), query.to_string(), true))
            .collect();
        presets.extend(
            self.custom_note_presets
                .iter()
                .cloned()
                .map(|preset| (preset.name, preset.query, false)),
        );
        presets
    }

    pub fn clear_note_form_message(&mut self) {
        self.note_form_message = None;
    }

    pub fn set_note_form_message<T: Into<String>>(&mut self, message: T) {
        self.note_form_message = Some(message.into());
    }

    pub fn mark_note_form_dirty(&mut self) {
        if matches!(
            self.input_mode,
            InputMode::AddingNote | InputMode::EditingNote
        ) {
            self.note_form_dirty = true;
            self.note_form_last_change_at = Some(Instant::now());
        }
    }

    pub fn begin_add_note(&mut self) {
        self.reset_inputs();
        self.input_mode = InputMode::AddingNote;
    }

    pub fn cancel_note_edit(&mut self) {
        self.reset_inputs();
        self.input_mode = InputMode::Normal;
    }

    pub fn begin_edit_note(&mut self) {
        if let Some(note) = self.notes.get(self.selected) {
            self.title_input = note.title.clone();
            self.content_input = note.content.clone();
            self.note_form_dirty = false;
            self.note_form_last_change_at = None;
            self.editing_title = true;
            self.input_mode = InputMode::EditingNote;
        } else {
            self.add_log("WARN", "No note selected to edit");
        }
    }

    pub fn begin_delete_note(&mut self) {
        if self.notes.is_empty() {
            self.add_log("WARN", "No note selected to delete");
            return;
        }
        self.input_mode = InputMode::DeleteNote;
    }

    pub fn maybe_autosave_note_edit(&mut self) -> Result<(), Box<dyn Error>> {
        if self.input_mode != InputMode::EditingNote || !self.note_form_dirty {
            return Ok(());
        }

        let Some(last_change) = self.note_form_last_change_at else {
            return Ok(());
        };
        if last_change.elapsed() < AUTOSAVE_IDLE_DELAY {
            return Ok(());
        }

        let Some(note_id) = self.notes.get(self.selected).map(|note| note.id) else {
            return Ok(());
        };

        let title = self.title_input.clone();
        let content = self.content_input.clone();
        self.update_note(note_id, &title, &content)?;
        self.note_form_dirty = false;
        self.note_form_last_change_at = None;
        self.note_form_message = Some(format!("Autosaved at {}", Local::now().format("%H:%M:%S")));
        Ok(())
    }

    pub fn maybe_autosave(&mut self) -> Result<(), Box<dyn Error>> {
        self.maybe_autosave_note_edit()?;
        self.maybe_autosave_inline_file_edit()?;
        Ok(())
    }
}
