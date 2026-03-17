use std::{error::Error, io};

use crate::filter_presets::{save_presets, SavedFilterPreset};

use super::{App, InputMode};

impl App {
    pub fn begin_note_presets(&mut self) {
        self.preset_selected = 0;
        self.input_mode = InputMode::PresetFilters;
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

    pub fn apply_selected_note_preset(&mut self) {
        if let Some((name, query, _)) = self
            .all_note_filter_presets()
            .get(self.preset_selected)
            .cloned()
        {
            self.note_filter = query;
            self.ensure_selected_visible();
            self.add_log("INFO", &format!("Applied preset: {}", name));
        }
    }

    pub fn clear_preset_form(&mut self) {
        self.preset_name_input.clear();
        self.preset_form_message = None;
    }

    pub fn begin_save_note_preset(&mut self) {
        if self.note_filter.trim().is_empty() {
            self.add_log("WARN", "Set a note filter before saving a preset");
            return;
        }
        self.clear_preset_form();
        self.input_mode = InputMode::SavingPreset;
    }

    pub fn save_named_note_preset(&mut self) -> Result<(), Box<dyn Error>> {
        let name = self.preset_name_input.trim();
        if name.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Preset name cannot be empty").into(),
            );
        }
        let query = self.note_filter.trim();
        if query.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Filter query cannot be empty",
            )
            .into());
        }

        if let Some(existing) = self
            .custom_note_presets
            .iter_mut()
            .find(|preset| preset.name.eq_ignore_ascii_case(name))
        {
            existing.query = query.to_string();
        } else {
            self.custom_note_presets.push(SavedFilterPreset {
                name: name.to_string(),
                query: query.to_string(),
            });
        }
        save_presets(&self.preset_store_path, &self.custom_note_presets)?;
        self.add_log("INFO", &format!("Saved preset: {}", name));
        Ok(())
    }

    pub fn delete_selected_note_preset(&mut self) -> Result<bool, Box<dyn Error>> {
        let builtin_len = self.note_filter_presets().len();
        if self.preset_selected < builtin_len {
            return Ok(false);
        }

        let custom_index = self.preset_selected - builtin_len;
        if custom_index < self.custom_note_presets.len() {
            let removed = self.custom_note_presets.remove(custom_index);
            save_presets(&self.preset_store_path, &self.custom_note_presets)?;
            self.add_log("INFO", &format!("Deleted preset: {}", removed.name));
            if self.preset_selected > 0
                && self.preset_selected >= self.all_note_filter_presets().len()
            {
                self.preset_selected -= 1;
            }
            return Ok(true);
        }
        Ok(false)
    }
}
