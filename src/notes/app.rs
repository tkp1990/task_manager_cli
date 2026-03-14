use chrono::Local;
use std::{error::Error, io};

use crate::db::notes::models::{Note, NoteUpdate};
use crate::db::notes::operations::DbOperations;

/// The mode of the application: either in normal navigation or editing.
#[derive(Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Filtering,
    PresetFilters,
    AddingNote,
    EditingNote,
    ViewingNote,
    DeleteNote,
    Help,
}

/// The overall application state.
pub struct App {
    /// Database operations handler
    pub db_ops: DbOperations,
    /// Current list of notes.
    pub notes: Vec<Note>,
    /// Current filter query for the visible note list.
    pub note_filter: String,
    /// Current selected preset in the presets popup.
    pub preset_selected: usize,
    /// Currently selected index in the notes list.
    pub selected: usize,
    /// The current input mode.
    pub input_mode: InputMode,
    /// Buffer for note title input.
    pub title_input: String,
    /// Buffer for note content input.
    pub content_input: String,
    /// Inline feedback shown inside the note form popup.
    pub note_form_message: Option<String>,
    /// Track if we're editing title (true) or content (false)
    pub editing_title: bool,
    /// Log storage.
    pub logs: Vec<String>,
    /// Scroll offset to be displayed.
    pub log_offset: usize,
}

impl App {
    /// Create a new App instance.
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
            notes: Vec::new(),
            note_filter: String::new(),
            preset_selected: 0,
            selected: 0,
            input_mode: InputMode::Normal,
            title_input: String::new(),
            content_input: String::new(),
            note_form_message: None,
            editing_title: true,
            logs: Vec::new(),
            log_offset: 0,
        };
        app.load_notes()?;
        app.add_log("INFO", "Notes loaded");
        app.add_log("INFO", "Application started");
        Ok(app)
    }

    /// Load notes from the database.
    pub fn load_notes(&mut self) -> Result<(), Box<dyn Error>> {
        self.notes = self.db_ops.load_notes()?;

        // Bounds check for selected note
        self.ensure_selected_visible();
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

    /// Add a new note to the database.
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

    /// Update an existing note.
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

    /// Delete the currently selected note.
    pub fn delete_note(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(note) = self.notes.get(self.selected) {
            self.db_ops.delete_note(note.id)?;
            self.add_log("INFO", &format!("Deleted note id: {}", note.id));
            self.load_notes()?;
            // Adjust selected index if needed.
            if self.selected > 0 && self.selected >= self.notes.len() {
                self.selected -= 1;
            }
        }
        Ok(())
    }

    // Reset note input fields
    pub fn reset_inputs(&mut self) {
        self.title_input.clear();
        self.content_input.clear();
        self.note_form_message = None;
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
        if let Some((name, query)) = self
            .note_filter_presets()
            .get(self.preset_selected)
            .copied()
        {
            self.note_filter = query.to_string();
            self.ensure_selected_visible();
            self.add_log("INFO", &format!("Applied preset: {}", name));
        }
    }

    pub fn clear_note_form_message(&mut self) {
        self.note_form_message = None;
    }

    pub fn set_note_form_message<T: Into<String>>(&mut self, message: T) {
        self.note_form_message = Some(message.into());
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
        std::env::temp_dir().join(format!("task_manager_cli_notes_app_{unique}.db"))
    }

    #[test]
    fn begin_add_note_resets_inputs() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("begin_add");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.title_input = "stale".to_string();
        app.content_input = "stale".to_string();
        app.editing_title = false;

        app.begin_add_note();

        assert_eq!(app.input_mode, InputMode::AddingNote);
        assert!(app.title_input.is_empty());
        assert!(app.content_input.is_empty());
        assert!(app.editing_title);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn begin_edit_note_prefills_existing_content() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("begin_edit");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_note("Title", "Body")?;
        app.begin_edit_note();

        assert_eq!(app.input_mode, InputMode::EditingNote);
        assert_eq!(app.title_input, "Title");
        assert_eq!(app.content_input, "Body");
        assert!(app.editing_title);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn add_note_rejects_blank_titles() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("blank_title");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        let error = app.add_note("   ", "Body").unwrap_err();
        assert!(error.to_string().contains("Note title cannot be empty"));

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn reset_inputs_clears_inline_feedback() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("feedback");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.title_input = "Draft".to_string();
        app.content_input = "Draft body".to_string();
        app.set_note_form_message("Note title cannot be empty");

        app.reset_inputs();

        assert!(app.title_input.is_empty());
        assert!(app.content_input.is_empty());
        assert!(app.note_form_message.is_none());

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn note_filter_keeps_selection_on_visible_match() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("filter");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_note("Alpha", "First body")?;
        app.add_note("Beta", "Second body")?;
        app.selected = 1;
        app.note_filter = "Alpha".to_string();
        app.ensure_selected_visible();

        assert_eq!(app.filtered_note_indices(), vec![1]);
        assert_eq!(app.selected, 1);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn note_filter_supports_title_and_body_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("note_filter_tokens");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_note("Project Alpha", "Meeting notes")?;
        app.add_note("Shopping", "Buy apples and bread")?;

        app.note_filter = "title:project".to_string();
        assert_eq!(app.filtered_note_indices(), vec![1]);

        app.note_filter = "body:apples".to_string();
        assert_eq!(app.filtered_note_indices(), vec![0]);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn note_filter_supports_phrases_and_negation() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("note_filter_phrases");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_note("Project Alpha", "Roadmap review notes")?;
        app.add_note("Shopping", "Buy apples and bread")?;

        app.note_filter = "title:\"Project Alpha\"".to_string();
        assert_eq!(app.filtered_note_indices(), vec![1]);

        app.note_filter = "\"buy apples\" -title:shopping".to_string();
        assert!(app.filtered_note_indices().is_empty());

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn applying_note_preset_sets_filter_and_selection() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("note_preset");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new(&db_path_str)?;

        app.add_note("Project Alpha", "Meeting notes")?;
        app.add_note("Shopping", "Buy apples and bread")?;
        app.selected = 0;

        app.preset_selected = 2;
        app.apply_selected_note_preset();

        assert_eq!(app.note_filter, "title:shopping");
        assert_eq!(app.filtered_note_indices(), vec![0]);
        assert_eq!(app.selected, 0);

        let _ = fs::remove_file(db_path);
        Ok(())
    }
}
