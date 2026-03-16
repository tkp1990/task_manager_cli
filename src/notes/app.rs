use chrono::Local;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::db::notes::models::{Note, NoteUpdate};
use crate::db::notes::operations::DbOperations;
use crate::filter_presets::{load_presets, save_presets, SavedFilterPreset};

/// The mode of the application: either in normal navigation or editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    CommandPalette,
    Filtering,
    SearchingFiles,
    FileShortcuts,
    FileLinks,
    PresetFilters,
    SavingPreset,
    AddingNote,
    EditingNote,
    ViewingNote,
    ViewingFile,
    EditingFile,
    CreatingFile,
    CreatingDirectory,
    RenamingFileEntry,
    MovingFileEntry,
    CopyingFileEntry,
    DeletingFileEntry,
    DeleteNote,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesView {
    Files,
    Database,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedFileShortcut {
    pub name: String,
    pub target: String,
    pub kind: FileShortcutKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileShortcutKind {
    Directory,
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteReference {
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDefinition {
    pub name: String,
    pub content: String,
    pub is_custom: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileMetadata {
    pub title: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedFileLink {
    pub group: &'static str,
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTemplate {
    Blank,
    DailyNote,
    MeetingNote,
    ProjectNote,
    JournalEntry,
}

impl FileTemplate {
    pub fn name(&self) -> &'static str {
        match self {
            FileTemplate::Blank => "Blank",
            FileTemplate::DailyNote => "Daily Note",
            FileTemplate::MeetingNote => "Meeting Note",
            FileTemplate::ProjectNote => "Project Note",
            FileTemplate::JournalEntry => "Journal Entry",
        }
    }
}

/// The overall application state.
pub struct App {
    /// Database operations handler
    pub db_ops: DbOperations,
    /// Current list of notes.
    pub notes: Vec<Note>,
    /// Current top-level notes experience.
    pub active_view: NotesView,
    /// Root directory for file-backed notes.
    pub notes_root: PathBuf,
    /// Currently displayed directory in the file browser.
    pub current_dir: PathBuf,
    /// Visible file entries for the current directory.
    pub file_entries: Vec<FileEntry>,
    /// Recursive fuzzy-find query for the file tree.
    pub file_search_query: String,
    /// Search results when fuzzy find is active.
    pub file_search_results: Vec<FileEntry>,
    /// Saved pinned directories and saved file searches.
    pub file_shortcuts: Vec<SavedFileShortcut>,
    /// Current selection inside the shortcuts popup.
    pub file_shortcut_selected: usize,
    /// Current selection inside the related-links popup.
    pub file_link_selected: usize,
    /// Whether the links panel is focused in the main file view.
    pub file_view_links_focus: bool,
    /// Selected entry in the file browser.
    pub file_selected: usize,
    /// Cached preview path for the selected entry.
    pub previewed_file_path: Option<PathBuf>,
    /// Cached preview content shown in the browser side pane.
    pub previewed_file_content: String,
    /// Vertical scroll offset for the browser preview pane.
    pub preview_scroll: usize,
    /// Currently opened file path.
    pub viewed_file_path: Option<PathBuf>,
    /// Content for the currently opened file.
    pub viewed_file_content: String,
    /// Vertical scroll offset for the full file view content pane.
    pub viewed_file_scroll: usize,
    /// Optional editor command override used for file editing.
    pub editor_command: Option<String>,
    /// Path for persisted file shortcuts.
    pub file_shortcuts_store_path: PathBuf,
    /// Directory containing user-defined templates.
    pub templates_dir: PathBuf,
    /// Target entry for rename/delete operations.
    pub pending_file_path: Option<PathBuf>,
    /// Buffer for file and directory actions.
    pub file_name_input: String,
    /// Inline feedback for file actions.
    pub file_form_message: Option<String>,
    /// Selected built-in template for file creation.
    pub file_template_selected: usize,
    /// User-defined file templates loaded from disk.
    pub custom_file_templates: Vec<TemplateDefinition>,
    /// Buffer for inline file editing.
    pub file_edit_content: String,
    /// Inline feedback shown inside the file editor.
    pub file_edit_message: Option<String>,
    /// Cursor row within the inline editor.
    pub file_edit_cursor_row: usize,
    /// Cursor column within the inline editor.
    pub file_edit_cursor_col: usize,
    /// Top-most visible line in the inline editor.
    pub file_edit_scroll: usize,
    /// Left-most visible column in the inline editor.
    pub file_edit_scroll_x: usize,
    /// Preferred column preserved while moving vertically.
    pub file_edit_preferred_col: usize,
    /// Current filter query for the visible note list.
    pub note_filter: String,
    /// Current selected preset in the presets popup.
    pub preset_selected: usize,
    /// User-defined presets persisted to disk.
    pub custom_note_presets: Vec<SavedFilterPreset>,
    /// Preset storage path.
    pub preset_store_path: PathBuf,
    /// Palette history storage path.
    pub palette_history_store_path: PathBuf,
    /// Currently selected index in the notes list.
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
    /// Buffer for note title input.
    pub title_input: String,
    /// Buffer for note content input.
    pub content_input: String,
    /// Inline feedback shown inside the note form popup.
    pub note_form_message: Option<String>,
    /// Buffer for naming a saved preset.
    pub preset_name_input: String,
    /// Inline feedback shown inside the preset popup.
    pub preset_form_message: Option<String>,
    /// Track if we're editing title (true) or content (false)
    pub editing_title: bool,
    /// Log storage.
    pub logs: Vec<String>,
    /// Scroll offset to be displayed.
    pub log_offset: usize,
}

impl App {
    pub fn new_with_notes_root(db_path: &str, notes_root: PathBuf) -> Result<App, Box<dyn Error>> {
        let db_file_path = PathBuf::from(db_path);
        let db_stem = db_file_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("notes");
        let db_path_string = format!("sqlite://{}", db_path);
        let pool = crate::db::establish_connection_pool(&db_path_string)?;
        let preset_store_path = db_file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{db_stem}.note_filter_presets.json"));
        let file_shortcuts_store_path = db_file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{db_stem}.note_file_shortcuts.json"));
        let palette_history_store_path = db_file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{db_stem}.note_palette_history.json"));
        let templates_dir = notes_root
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("templates");
        fs::create_dir_all(&notes_root)?;
        fs::create_dir_all(&templates_dir)?;

        // Run migrations if needed
        {
            let mut conn = pool.get()?;
            crate::db::run_migrations(&mut conn)?;
        }

        let db_ops = DbOperations::new(pool);
        let mut app = App {
            db_ops,
            notes: Vec::new(),
            active_view: NotesView::Files,
            current_dir: notes_root.clone(),
            notes_root,
            file_entries: Vec::new(),
            file_search_query: String::new(),
            file_search_results: Vec::new(),
            file_shortcuts: load_file_shortcuts(&file_shortcuts_store_path)?,
            file_shortcut_selected: 0,
            file_link_selected: 0,
            file_view_links_focus: false,
            file_selected: 0,
            previewed_file_path: None,
            previewed_file_content: String::new(),
            preview_scroll: 0,
            viewed_file_path: None,
            viewed_file_content: String::new(),
            viewed_file_scroll: 0,
            editor_command: None,
            file_shortcuts_store_path,
            templates_dir: templates_dir.clone(),
            pending_file_path: None,
            file_name_input: String::new(),
            file_form_message: None,
            file_template_selected: 0,
            custom_file_templates: load_custom_templates(&templates_dir)?,
            file_edit_content: String::new(),
            file_edit_message: None,
            file_edit_cursor_row: 0,
            file_edit_cursor_col: 0,
            file_edit_scroll: 0,
            file_edit_scroll_x: 0,
            file_edit_preferred_col: 0,
            note_filter: String::new(),
            preset_selected: 0,
            custom_note_presets: load_presets(&preset_store_path)?,
            preset_store_path,
            palette_history_store_path: palette_history_store_path.clone(),
            selected: 0,
            input_mode: InputMode::Normal,
            command_palette_return_mode: InputMode::Normal,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            recent_palette_commands: load_palette_history(&palette_history_store_path)?,
            title_input: String::new(),
            content_input: String::new(),
            note_form_message: None,
            preset_name_input: String::new(),
            preset_form_message: None,
            editing_title: true,
            logs: Vec::new(),
            log_offset: 0,
        };
        app.load_notes()?;
        app.load_file_entries()?;
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

    pub fn toggle_active_view(&mut self) {
        self.input_mode = InputMode::Normal;
        self.active_view = match self.active_view {
            NotesView::Files => NotesView::Database,
            NotesView::Database => NotesView::Files,
        };
    }

    pub fn load_file_entries(&mut self) -> Result<(), Box<dyn Error>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.current_dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            let metadata = entry.metadata()?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path,
                is_dir: file_type.is_dir(),
                size_bytes: metadata.len(),
                modified_at: metadata
                    .modified()
                    .ok()
                    .map(|time| chrono::DateTime::<chrono::Local>::from(time))
                    .map(|time| time.format("%Y-%m-%d %H:%M").to_string()),
            });
        }

        entries.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });

        self.file_entries = entries;
        self.ensure_file_selection_visible();
        self.refresh_file_search_results()?;
        self.sync_file_preview()?;
        Ok(())
    }

    pub fn refresh_file_browser(&mut self) -> Result<(), Box<dyn Error>> {
        self.load_file_entries()
    }

    pub fn move_file_selection_down(&mut self) {
        if self.file_selected + 1 < self.visible_file_entries().len() {
            self.file_selected += 1;
            let _ = self.sync_file_preview();
        }
    }

    pub fn move_file_selection_up(&mut self) {
        if self.file_selected > 0 {
            self.file_selected -= 1;
            let _ = self.sync_file_preview();
        }
    }

    pub fn has_file_search(&self) -> bool {
        !self.file_search_query.trim().is_empty()
    }

    pub fn begin_file_search(&mut self) {
        self.input_mode = InputMode::SearchingFiles;
    }

    pub fn begin_file_shortcuts(&mut self) {
        self.file_shortcut_selected = 0;
        self.input_mode = InputMode::FileShortcuts;
    }

    pub fn begin_file_links(&mut self) {
        self.file_link_selected = 0;
        self.input_mode = InputMode::FileLinks;
    }

    pub fn append_file_search_char(&mut self, c: char) {
        self.file_search_query.push(c);
        let _ = self.refresh_file_search_results();
        self.ensure_file_selection_visible();
        let _ = self.sync_file_preview();
    }

    pub fn pop_file_search_char(&mut self) {
        self.file_search_query.pop();
        let _ = self.refresh_file_search_results();
        self.ensure_file_selection_visible();
        let _ = self.sync_file_preview();
    }

    pub fn clear_file_search(&mut self) {
        self.file_search_query.clear();
        self.file_search_results.clear();
        self.file_selected = 0;
        let _ = self.sync_file_preview();
    }

    pub fn set_file_search_query(&mut self, query: &str) -> Result<(), Box<dyn Error>> {
        self.file_search_query = query.to_string();
        self.refresh_file_search_results()?;
        self.ensure_file_selection_visible();
        self.sync_file_preview()?;
        Ok(())
    }

    pub fn visible_file_entries(&self) -> &[FileEntry] {
        if self.has_file_search() {
            &self.file_search_results
        } else {
            &self.file_entries
        }
    }

    pub fn ensure_file_selection_visible(&mut self) {
        let len = self.visible_file_entries().len();
        if len == 0 {
            self.file_selected = 0;
        } else if self.file_selected >= len {
            self.file_selected = len - 1;
        }
    }

    pub fn selected_file_entry(&self) -> Option<&FileEntry> {
        self.visible_file_entries().get(self.file_selected)
    }

    pub fn begin_inline_file_edit(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(entry) = self.selected_file_entry().cloned() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file selected").into());
        };
        if entry.is_dir {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Directories cannot be edited inline",
            )
            .into());
        }

        self.viewed_file_path = Some(entry.path.clone());
        self.file_edit_content = fs::read_to_string(&entry.path)?;
        self.file_edit_message = None;
        self.file_edit_cursor_row = self.editor_lines().len().saturating_sub(1);
        self.file_edit_cursor_col = self
            .editor_lines()
            .last()
            .map(|line| line.chars().count())
            .unwrap_or(0);
        self.file_edit_preferred_col = self.file_edit_cursor_col;
        self.file_edit_scroll = self.file_edit_cursor_row.saturating_sub(3);
        self.file_edit_scroll_x = self.file_edit_cursor_col.saturating_sub(20);
        self.input_mode = InputMode::EditingFile;
        Ok(())
    }

    pub fn save_inline_file_edit(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.viewed_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file is open").into());
        };
        fs::write(&path, self.file_edit_content.as_bytes())?;
        self.load_file_entries()?;
        self.select_entry_path(&path);
        self.viewed_file_content = self.file_edit_content.clone();
        self.previewed_file_path = Some(path.clone());
        self.previewed_file_content = render_preview_content(&path, &self.file_edit_content);
        self.file_edit_message = Some("Saved".to_string());
        self.add_log("INFO", &format!("Saved file: {}", path.display()));
        self.input_mode = InputMode::ViewingFile;
        Ok(())
    }

    pub fn cancel_inline_file_edit(&mut self) {
        self.file_edit_content.clear();
        self.file_edit_message = None;
        self.file_edit_cursor_row = 0;
        self.file_edit_cursor_col = 0;
        self.file_edit_scroll = 0;
        self.file_edit_scroll_x = 0;
        self.file_edit_preferred_col = 0;
        self.input_mode = InputMode::ViewingFile;
    }

    pub fn inline_editor_lines(&self) -> Vec<String> {
        self.editor_lines()
    }

    pub fn inline_editor_preview(&self) -> String {
        self.viewed_file_path
            .as_ref()
            .map(|path| render_preview_content(path, &self.file_edit_content))
            .unwrap_or_else(|| self.file_edit_content.clone())
    }

    pub fn move_file_edit_left(&mut self) {
        if self.file_edit_cursor_col > 0 {
            self.file_edit_cursor_col -= 1;
        } else if self.file_edit_cursor_row > 0 {
            self.file_edit_cursor_row -= 1;
            self.file_edit_cursor_col = self.current_line_len();
        }
        self.file_edit_preferred_col = self.file_edit_cursor_col;
    }

    pub fn move_file_edit_right(&mut self) {
        let current_len = self.current_line_len();
        if self.file_edit_cursor_col < current_len {
            self.file_edit_cursor_col += 1;
        } else if self.file_edit_cursor_row + 1 < self.editor_lines().len() {
            self.file_edit_cursor_row += 1;
            self.file_edit_cursor_col = 0;
        }
        self.file_edit_preferred_col = self.file_edit_cursor_col;
    }

    pub fn move_file_edit_up(&mut self) {
        if self.file_edit_cursor_row > 0 {
            self.file_edit_cursor_row -= 1;
            self.file_edit_cursor_col = self.file_edit_preferred_col.min(self.current_line_len());
        }
    }

    pub fn move_file_edit_down(&mut self) {
        if self.file_edit_cursor_row + 1 < self.editor_lines().len() {
            self.file_edit_cursor_row += 1;
            self.file_edit_cursor_col = self.file_edit_preferred_col.min(self.current_line_len());
        }
    }

    pub fn scroll_file_edit_up(&mut self) {
        if self.file_edit_scroll > 0 {
            self.file_edit_scroll -= 1;
        }
    }

    pub fn scroll_file_edit_down(&mut self) {
        if self.file_edit_scroll + 1 < self.editor_lines().len() {
            self.file_edit_scroll += 1;
        }
    }

    pub fn ensure_file_edit_cursor_visible(&mut self, height: usize, width: usize) {
        if self.file_edit_cursor_row < self.file_edit_scroll {
            self.file_edit_scroll = self.file_edit_cursor_row;
        } else if self.file_edit_cursor_row >= self.file_edit_scroll + height {
            self.file_edit_scroll = self
                .file_edit_cursor_row
                .saturating_sub(height.saturating_sub(1));
        }
        if self.file_edit_cursor_col < self.file_edit_scroll_x {
            self.file_edit_scroll_x = self.file_edit_cursor_col;
        } else if self.file_edit_cursor_col >= self.file_edit_scroll_x + width {
            self.file_edit_scroll_x = self
                .file_edit_cursor_col
                .saturating_sub(width.saturating_sub(1));
        }
    }

    pub fn insert_file_edit_char(&mut self, c: char) {
        let mut lines = self.editor_lines();
        while self.file_edit_cursor_row >= lines.len() {
            lines.push(String::new());
        }
        let row = self.file_edit_cursor_row;
        let col = self.file_edit_cursor_col.min(lines[row].chars().count());
        let byte_idx = char_to_byte_idx(&lines[row], col);
        lines[row].insert(byte_idx, c);
        self.file_edit_cursor_col += 1;
        self.file_edit_preferred_col = self.file_edit_cursor_col;
        self.file_edit_content = lines.join("\n");
        self.file_edit_message = None;
    }

    pub fn insert_file_edit_newline(&mut self) {
        let mut lines = self.editor_lines();
        while self.file_edit_cursor_row >= lines.len() {
            lines.push(String::new());
        }
        let row = self.file_edit_cursor_row;
        let col = self.file_edit_cursor_col.min(lines[row].chars().count());
        let byte_idx = char_to_byte_idx(&lines[row], col);
        let remainder = lines[row][byte_idx..].to_string();
        lines[row].truncate(byte_idx);
        lines.insert(row + 1, remainder);
        self.file_edit_cursor_row += 1;
        self.file_edit_cursor_col = 0;
        self.file_edit_preferred_col = 0;
        self.file_edit_content = lines.join("\n");
        self.file_edit_message = None;
    }

    pub fn insert_file_edit_tab(&mut self) {
        for _ in 0..4 {
            self.insert_file_edit_char(' ');
        }
    }

    pub fn backspace_file_edit(&mut self) {
        let mut lines = self.editor_lines();
        if lines.is_empty() {
            lines.push(String::new());
        }

        if self.file_edit_cursor_col > 0 {
            let row = self.file_edit_cursor_row.min(lines.len().saturating_sub(1));
            let col = self.file_edit_cursor_col.min(lines[row].chars().count());
            let end = char_to_byte_idx(&lines[row], col);
            let start = char_to_byte_idx(&lines[row], col - 1);
            lines[row].replace_range(start..end, "");
            self.file_edit_cursor_col -= 1;
            self.file_edit_preferred_col = self.file_edit_cursor_col;
        } else if self.file_edit_cursor_row > 0 {
            let row = self.file_edit_cursor_row.min(lines.len().saturating_sub(1));
            let previous_len = lines[row - 1].chars().count();
            let current = lines.remove(row);
            lines[row - 1].push_str(&current);
            self.file_edit_cursor_row -= 1;
            self.file_edit_cursor_col = previous_len;
            self.file_edit_preferred_col = previous_len;
        }

        self.file_edit_content = lines.join("\n");
        self.file_edit_message = None;
    }

    fn editor_lines(&self) -> Vec<String> {
        if self.file_edit_content.is_empty() {
            vec![String::new()]
        } else {
            self.file_edit_content
                .lines()
                .map(|line| line.to_string())
                .collect()
        }
    }

    fn current_line_len(&self) -> usize {
        self.editor_lines()
            .get(self.file_edit_cursor_row)
            .map(|line| line.chars().count())
            .unwrap_or(0)
    }

    pub fn open_selected_file_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(entry) = self.selected_file_entry().cloned() else {
            return Ok(());
        };

        if entry.is_dir {
            self.current_dir = entry.path;
            self.file_selected = 0;
            self.clear_file_search();
            self.load_file_entries()?;
            self.add_log(
                "INFO",
                &format!("Opened directory: {}", self.current_dir.display()),
            );
            return Ok(());
        }

        self.open_file_path(&entry.path)
    }

    pub fn open_file_path(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        let bytes = fs::read(path)?;
        self.current_dir = path.parent().unwrap_or(&self.notes_root).to_path_buf();
        self.clear_file_search();
        self.load_file_entries()?;
        self.select_entry_path(path);
        self.viewed_file_content = String::from_utf8_lossy(&bytes).into_owned();
        self.viewed_file_path = Some(path.to_path_buf());
        self.viewed_file_scroll = 0;
        self.file_link_selected = 0;
        self.file_view_links_focus = false;
        self.input_mode = InputMode::ViewingFile;
        self.add_log("INFO", &format!("Opened file: {}", path.display()));
        Ok(())
    }

    pub fn move_to_parent_directory(&mut self) -> Result<(), Box<dyn Error>> {
        if self.current_dir == self.notes_root {
            return Ok(());
        }

        if let Some(parent) = self.current_dir.parent() {
            if parent.starts_with(&self.notes_root) {
                self.current_dir = parent.to_path_buf();
                self.file_selected = 0;
                self.clear_file_search();
                self.load_file_entries()?;
            }
        }
        Ok(())
    }

    pub fn relative_current_dir(&self) -> String {
        self.relative_path_from_root(&self.current_dir)
    }

    pub fn relative_path_from_root(&self, path: &Path) -> String {
        path.strip_prefix(&self.notes_root)
            .ok()
            .and_then(|relative| {
                let display = relative.display().to_string();
                if display.is_empty() {
                    None
                } else {
                    Some(format!("/{}", display))
                }
            })
            .unwrap_or_else(|| "/".to_string())
    }

    pub fn selected_file_breadcrumb(&self) -> String {
        self.selected_file_entry()
            .map(|entry| self.relative_path_from_root(&entry.path))
            .unwrap_or_else(|| "/".to_string())
    }

    pub fn preview_summary(&self) -> String {
        if let Some(path) = &self.previewed_file_path {
            if path.is_dir() {
                "Directory preview".to_string()
            } else {
                let lines = self.previewed_file_content.lines().count();
                let backlink_count = self.file_backlinks(path).len();
                let metadata = self.file_metadata(path);
                let title_summary = metadata
                    .title
                    .as_deref()
                    .map(|title| format!("title: {title}"))
                    .unwrap_or_else(|| "no frontmatter title".to_string());
                let tag_summary = if metadata.tags.is_empty() {
                    "no tags".to_string()
                } else {
                    format!("tags: {}", metadata.tags.join(", "))
                };
                format!(
                    "{lines} lines | {backlink_count} backlinks | {title_summary} | {tag_summary}"
                )
            }
        } else {
            "No selection".to_string()
        }
    }

    pub fn file_metadata(&self, path: &Path) -> FileMetadata {
        let Ok(content) = fs::read_to_string(path) else {
            return FileMetadata::default();
        };
        parse_file_metadata(&content)
    }

    pub fn viewed_file_metadata(&self) -> FileMetadata {
        self.viewed_file_path
            .as_ref()
            .map(|path| self.file_metadata(path))
            .unwrap_or_default()
    }

    pub fn preview_line_count(&self) -> usize {
        line_count(&self.previewed_file_content)
    }

    pub fn viewed_file_line_count(&self) -> usize {
        line_count(&self.viewed_file_content)
    }

    pub fn scroll_preview_up(&mut self, amount: usize) {
        self.preview_scroll = self.preview_scroll.saturating_sub(amount);
    }

    pub fn scroll_preview_down(&mut self, amount: usize) {
        self.preview_scroll = self
            .preview_scroll
            .saturating_add(amount)
            .min(self.preview_line_count().saturating_sub(1));
    }

    pub fn scroll_viewed_file_up(&mut self, amount: usize) {
        self.viewed_file_scroll = self.viewed_file_scroll.saturating_sub(amount);
    }

    pub fn scroll_viewed_file_down(&mut self, amount: usize) {
        self.viewed_file_scroll = self
            .viewed_file_scroll
            .saturating_add(amount)
            .min(self.viewed_file_line_count().saturating_sub(1));
    }

    pub fn file_references(&self, path: &Path) -> Vec<NoteReference> {
        let Ok(content) = fs::read_to_string(path) else {
            return Vec::new();
        };
        extract_note_references(&self.notes_root, path, &content)
    }

    pub fn file_backlinks(&self, target: &Path) -> Vec<NoteReference> {
        let Ok(entries) = Self::collect_recursive_entries(&self.notes_root) else {
            return Vec::new();
        };
        let mut backlinks = Vec::new();

        for entry in entries.into_iter().filter(|entry| !entry.is_dir) {
            let Ok(content) = fs::read_to_string(&entry.path) else {
                continue;
            };
            let refs = extract_note_references(&self.notes_root, &entry.path, &content);
            if refs.iter().any(|reference| reference.path == target) {
                backlinks.push(NoteReference {
                    label: self.relative_path_from_root(&entry.path),
                    path: entry.path,
                });
            }
        }

        backlinks.sort_by(|left, right| left.label.cmp(&right.label));
        backlinks
    }

    pub fn related_file_links(&self) -> Vec<RelatedFileLink> {
        let Some(path) = self.viewed_file_path.as_ref() else {
            return Vec::new();
        };
        let mut links = Vec::new();
        links.extend(
            self.file_references(path)
                .into_iter()
                .map(|reference| RelatedFileLink {
                    group: "References",
                    label: reference.label,
                    path: reference.path,
                }),
        );
        links.extend(
            self.file_backlinks(path)
                .into_iter()
                .map(|reference| RelatedFileLink {
                    group: "Backlinks",
                    label: reference.label,
                    path: reference.path,
                }),
        );
        links
    }

    pub fn toggle_file_view_links_focus(&mut self) {
        self.file_view_links_focus = !self.file_view_links_focus;
    }

    pub fn move_file_link_down(&mut self) {
        if self.file_link_selected + 1 < self.related_file_links().len() {
            self.file_link_selected += 1;
        }
    }

    pub fn move_file_link_up(&mut self) {
        if self.file_link_selected > 0 {
            self.file_link_selected -= 1;
        }
    }

    pub fn open_selected_related_link(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(link) = self
            .related_file_links()
            .get(self.file_link_selected)
            .cloned()
        else {
            return Ok(());
        };
        self.open_file_path(&link.path)
    }

    pub fn begin_create_file(&mut self) {
        self.pending_file_path = None;
        self.file_name_input.clear();
        self.file_form_message = None;
        let _ = self.reload_custom_file_templates();
        self.file_template_selected = 0;
        self.input_mode = InputMode::CreatingFile;
    }

    pub fn begin_create_directory(&mut self) {
        self.pending_file_path = None;
        self.file_name_input.clear();
        self.file_form_message = None;
        self.input_mode = InputMode::CreatingDirectory;
    }

    pub fn clear_file_form_message(&mut self) {
        self.file_form_message = None;
    }

    pub fn set_file_form_message<T: Into<String>>(&mut self, message: T) {
        self.file_form_message = Some(message.into());
    }

    pub fn all_file_templates(&self) -> Vec<TemplateDefinition> {
        let mut templates = vec![
            TemplateDefinition {
                name: FileTemplate::Blank.name().to_string(),
                content: String::new(),
                is_custom: false,
            },
            TemplateDefinition {
                name: FileTemplate::DailyNote.name().to_string(),
                content: "# {{date}}\n\n## Goals\n\n- \n\n## Notes\n\n## Tasks\n\n- [ ] \n"
                    .to_string(),
                is_custom: false,
            },
            TemplateDefinition {
                name: FileTemplate::MeetingNote.name().to_string(),
                content: "# Meeting: {{title}}\n\nDate: {{date}}\nTime: {{time}}\nAttendees:\n\n## Agenda\n\n## Notes\n\n## Action Items\n\n- [ ] \n"
                    .to_string(),
                is_custom: false,
            },
            TemplateDefinition {
                name: FileTemplate::ProjectNote.name().to_string(),
                content: "# Project: {{title}}\n\nCreated: {{date}}\n\n## Summary\n\n## Milestones\n\n- \n\n## Open Questions\n\n- \n"
                    .to_string(),
                is_custom: false,
            },
            TemplateDefinition {
                name: FileTemplate::JournalEntry.name().to_string(),
                content: "# Journal - {{date}} ({{weekday}})\n\n## Mood\n\n## Highlights\n\n## Reflection\n\n"
                    .to_string(),
                is_custom: false,
            },
        ];
        templates.extend(self.custom_file_templates.clone());
        templates
    }

    pub fn selected_file_template_name(&self) -> String {
        self.all_file_templates()
            .get(self.file_template_selected)
            .map(|template| template.name.clone())
            .unwrap_or_else(|| FileTemplate::Blank.name().to_string())
    }

    pub fn move_file_template_down(&mut self) {
        if self.file_template_selected + 1 < self.all_file_templates().len() {
            self.file_template_selected += 1;
        }
    }

    pub fn move_file_template_up(&mut self) {
        if self.file_template_selected > 0 {
            self.file_template_selected -= 1;
        }
    }

    pub fn create_or_open_daily_note(&mut self) -> Result<(), Box<dyn Error>> {
        let daily_dir = self.notes_root.join("daily");
        fs::create_dir_all(&daily_dir)?;
        let file_name = format!("{}.md", Local::now().format("%Y-%m-%d"));
        let target = daily_dir.join(&file_name);
        if !target.exists() {
            let previous = self.file_template_selected;
            self.file_template_selected = 1;
            fs::write(&target, self.render_selected_template(&file_name))?;
            self.file_template_selected = previous;
            self.add_log("INFO", &format!("Created daily note: {}", target.display()));
        }
        self.open_file_path(&target)
    }

    pub fn cancel_file_creation(&mut self) {
        self.file_name_input.clear();
        self.file_form_message = None;
        self.pending_file_path = None;
        self.input_mode = InputMode::Normal;
    }

    pub fn create_file(&mut self) -> Result<(), Box<dyn Error>> {
        let final_name = self.normalized_child_name(true)?;
        let target = self.current_dir.join(&final_name);
        if target.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "File already exists").into());
        }

        fs::write(&target, self.render_selected_template(&final_name))?;
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log("INFO", &format!("Created file: {}", target.display()));
        self.cancel_file_creation();
        Ok(())
    }

    pub fn create_directory(&mut self) -> Result<(), Box<dyn Error>> {
        let final_name = self.normalized_child_name(false)?;
        let target = self.current_dir.join(&final_name);
        if target.exists() {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Directory already exists").into(),
            );
        }

        fs::create_dir(&target)?;
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log("INFO", &format!("Created directory: {}", target.display()));
        self.cancel_file_creation();
        Ok(())
    }

    pub fn begin_rename_selected_entry(&mut self) {
        if let Some(entry) = self.selected_file_entry().cloned() {
            self.pending_file_path = Some(entry.path.clone());
            self.file_name_input = entry.name.clone();
            self.file_form_message = None;
            self.input_mode = InputMode::RenamingFileEntry;
        } else {
            self.add_log("WARN", "No file entry selected to rename");
        }
    }

    pub fn begin_move_selected_entry(&mut self) {
        if let Some(entry) = self.selected_file_entry().cloned() {
            self.pending_file_path = Some(entry.path.clone());
            self.file_name_input = self
                .relative_path_from_root(&entry.path)
                .trim_start_matches('/')
                .to_string();
            self.file_form_message = None;
            self.input_mode = InputMode::MovingFileEntry;
        } else {
            self.add_log("WARN", "No file entry selected to move");
        }
    }

    pub fn begin_copy_selected_entry(&mut self) {
        if let Some(entry) = self.selected_file_entry().cloned() {
            self.pending_file_path = Some(entry.path.clone());
            self.file_name_input = self
                .relative_path_from_root(&entry.path)
                .trim_start_matches('/')
                .to_string();
            self.file_form_message = None;
            self.input_mode = InputMode::CopyingFileEntry;
        } else {
            self.add_log("WARN", "No file entry selected to copy");
        }
    }

    pub fn rename_selected_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(original_path) = self.pending_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file entry selected").into());
        };
        let is_file = original_path.is_file();
        let new_name = self.normalized_child_name(is_file)?;
        let target = original_path
            .parent()
            .unwrap_or(&self.current_dir)
            .join(new_name);
        if target == original_path {
            self.cancel_file_creation();
            return Ok(());
        }
        if target.exists() {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Target already exists").into(),
            );
        }

        fs::rename(&original_path, &target)?;
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log(
            "INFO",
            &format!(
                "Renamed {} to {}",
                original_path.display(),
                target
                    .file_name()
                    .unwrap_or_else(|| OsStr::new(""))
                    .to_string_lossy()
            ),
        );
        self.cancel_file_creation();
        Ok(())
    }

    pub fn begin_delete_selected_entry(&mut self) {
        if let Some(entry) = self.selected_file_entry().cloned() {
            self.pending_file_path = Some(entry.path.clone());
            self.input_mode = InputMode::DeletingFileEntry;
            self.file_form_message = None;
        } else {
            self.add_log("WARN", "No file entry selected to delete");
        }
    }

    pub fn delete_selected_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(target) = self.pending_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file entry selected").into());
        };
        if target.is_dir() {
            fs::remove_dir_all(&target)?;
        } else {
            fs::remove_file(&target)?;
        }
        self.load_file_entries()?;
        self.add_log("INFO", &format!("Deleted {}", target.display()));
        self.pending_file_path = None;
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn move_selected_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(source) = self.pending_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file entry selected").into());
        };
        let target = self.resolve_destination_path(&source)?;
        if target == source {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Destination is unchanged").into(),
            );
        }
        if target.exists() {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Destination already exists").into(),
            );
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&source, &target)?;
        if source.parent() != Some(&self.current_dir) {
            if let Some(parent) = target.parent() {
                self.current_dir = parent.to_path_buf();
            }
        }
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log(
            "INFO",
            &format!("Moved {} to {}", source.display(), target.display()),
        );
        self.cancel_file_creation();
        Ok(())
    }

    pub fn copy_selected_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(source) = self.pending_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file entry selected").into());
        };
        let target = self.resolve_destination_path(&source)?;
        if target == source {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Destination must differ from source",
            )
            .into());
        }
        if target.exists() {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Destination already exists").into(),
            );
        }
        copy_path_recursive(&source, &target)?;
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log(
            "INFO",
            &format!("Copied {} to {}", source.display(), target.display()),
        );
        self.cancel_file_creation();
        Ok(())
    }

    pub fn edit_selected_file_in_editor(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(entry) = self.selected_file_entry().cloned() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file selected").into());
        };
        self.edit_file_in_editor(&entry.path)
    }

    pub fn edit_viewed_file_in_editor(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.viewed_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file is open").into());
        };
        self.edit_file_in_editor(&path)
    }

    fn edit_file_in_editor(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        if path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Directories cannot be edited in an external editor",
            )
            .into());
        }
        let editor = self
            .editor_command
            .clone()
            .or_else(|| std::env::var("NOTES_EDITOR").ok())
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "nvim".to_string());
        let quoted_path = shell_quote(path);
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!("{editor} {quoted_path}"))
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!("Editor exited with status {status}")).into());
        }

        self.load_file_entries()?;
        self.select_entry_path(path);
        let bytes = fs::read(path)?;
        let content = String::from_utf8_lossy(&bytes).into_owned();
        self.viewed_file_path = Some(path.to_path_buf());
        self.viewed_file_content = content.clone();
        self.previewed_file_path = Some(path.to_path_buf());
        self.previewed_file_content = render_preview_content(path, &content);
        self.add_log("INFO", &format!("Edited file: {}", path.display()));
        Ok(())
    }

    pub fn render_selected_template(&self, file_name: &str) -> String {
        let title = Path::new(file_name)
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or(file_name);
        let now = Local::now();
        let template = self
            .all_file_templates()
            .get(self.file_template_selected)
            .map(|template| template.content.clone())
            .unwrap_or_default();

        [
            ("{{date}}", now.format("%Y-%m-%d").to_string()),
            ("{{time}}", now.format("%H:%M").to_string()),
            ("{{weekday}}", now.format("%A").to_string()),
            ("{{title}}", title.to_string()),
        ]
        .into_iter()
        .fold(template, |acc, (key, value)| acc.replace(key, &value))
    }

    pub fn reload_custom_file_templates(&mut self) -> Result<(), Box<dyn Error>> {
        self.custom_file_templates = load_custom_templates(&self.templates_dir)?;
        Ok(())
    }

    fn normalized_child_name(&self, default_markdown: bool) -> Result<String, Box<dyn Error>> {
        let trimmed = self.file_name_input.trim();
        if trimmed.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Name cannot be empty").into());
        }
        if trimmed.contains(std::path::MAIN_SEPARATOR) || trimmed.contains("..") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Name must stay within the current directory",
            )
            .into());
        }
        Ok(
            if default_markdown && Path::new(trimmed).extension().is_none() {
                format!("{trimmed}.md")
            } else {
                trimmed.to_string()
            },
        )
    }

    fn resolve_destination_path(&self, source: &Path) -> Result<PathBuf, Box<dyn Error>> {
        let raw = self.file_name_input.trim().trim_start_matches('/');
        if raw.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Destination cannot be empty").into(),
            );
        }
        if raw.contains("..") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Destination must stay within notes root",
            )
            .into());
        }

        let mut target = self.notes_root.join(raw);
        if target.exists() && target.is_dir() {
            target = target.join(source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Source has no file name")
            })?);
        }
        if !target.starts_with(&self.notes_root) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Destination must stay within notes root",
            )
            .into());
        }
        Ok(target)
    }

    fn select_entry_path(&mut self, target: &Path) {
        let target = target.to_path_buf();
        if self.has_file_search() {
            if let Some(index) = self
                .file_search_results
                .iter()
                .position(|entry| entry.path == target)
            {
                self.file_selected = index;
            }
        } else if let Some(index) = self
            .file_entries
            .iter()
            .position(|entry| entry.path == target)
        {
            self.file_selected = index;
        }
        let _ = self.sync_file_preview();
    }

    pub fn select_file_entry_path(&mut self, target: &Path) {
        self.select_entry_path(target);
    }

    pub fn all_file_shortcuts(&self) -> &[SavedFileShortcut] {
        &self.file_shortcuts
    }

    pub fn toggle_pin_current_directory(&mut self) -> Result<(), Box<dyn Error>> {
        let path_text = self.current_dir.to_string_lossy().to_string();
        if let Some(index) = self.file_shortcuts.iter().position(|shortcut| {
            shortcut.kind == FileShortcutKind::Directory && shortcut.target == path_text
        }) {
            let removed = self.file_shortcuts.remove(index);
            save_file_shortcuts(&self.file_shortcuts_store_path, &self.file_shortcuts)?;
            self.add_log(
                "INFO",
                &format!("Removed pinned directory: {}", removed.name),
            );
        } else {
            let name = if self.current_dir == self.notes_root {
                "Root".to_string()
            } else {
                self.current_dir
                    .file_name()
                    .unwrap_or_else(|| OsStr::new("directory"))
                    .to_string_lossy()
                    .to_string()
            };
            self.file_shortcuts.push(SavedFileShortcut {
                name,
                target: path_text,
                kind: FileShortcutKind::Directory,
            });
            save_file_shortcuts(&self.file_shortcuts_store_path, &self.file_shortcuts)?;
            self.add_log("INFO", "Pinned current directory");
        }
        Ok(())
    }

    pub fn save_current_file_search(&mut self) -> Result<(), Box<dyn Error>> {
        let query = self.file_search_query.trim();
        if query.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "No file search to save").into(),
            );
        }

        if let Some(existing) = self.file_shortcuts.iter_mut().find(|shortcut| {
            shortcut.kind == FileShortcutKind::Search && shortcut.name.eq_ignore_ascii_case(query)
        }) {
            existing.target = query.to_string();
        } else {
            self.file_shortcuts.push(SavedFileShortcut {
                name: query.to_string(),
                target: query.to_string(),
                kind: FileShortcutKind::Search,
            });
        }
        save_file_shortcuts(&self.file_shortcuts_store_path, &self.file_shortcuts)?;
        self.add_log("INFO", &format!("Saved search: {}", query));
        Ok(())
    }

    pub fn move_file_shortcut_down(&mut self) {
        if self.file_shortcut_selected + 1 < self.file_shortcuts.len() {
            self.file_shortcut_selected += 1;
        }
    }

    pub fn move_file_shortcut_up(&mut self) {
        if self.file_shortcut_selected > 0 {
            self.file_shortcut_selected -= 1;
        }
    }

    pub fn apply_selected_file_shortcut(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(shortcut) = self
            .file_shortcuts
            .get(self.file_shortcut_selected)
            .cloned()
        else {
            return Ok(());
        };
        match shortcut.kind {
            FileShortcutKind::Directory => {
                let target = PathBuf::from(shortcut.target);
                if target.exists() && target.starts_with(&self.notes_root) {
                    self.current_dir = target;
                    self.clear_file_search();
                    self.file_selected = 0;
                    self.load_file_entries()?;
                }
            }
            FileShortcutKind::Search => {
                self.set_file_search_query(&shortcut.target)?;
            }
        }
        self.add_log("INFO", &format!("Opened shortcut: {}", shortcut.name));
        Ok(())
    }

    pub fn delete_selected_file_shortcut(&mut self) -> Result<(), Box<dyn Error>> {
        if self.file_shortcut_selected < self.file_shortcuts.len() {
            let removed = self.file_shortcuts.remove(self.file_shortcut_selected);
            save_file_shortcuts(&self.file_shortcuts_store_path, &self.file_shortcuts)?;
            if self.file_shortcut_selected > 0
                && self.file_shortcut_selected >= self.file_shortcuts.len()
            {
                self.file_shortcut_selected -= 1;
            }
            self.add_log("INFO", &format!("Removed shortcut: {}", removed.name));
        }
        Ok(())
    }

    fn refresh_file_search_results(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.has_file_search() {
            self.file_search_results.clear();
            return Ok(());
        }

        let query = self.file_search_query.clone();
        let mut results = Self::collect_recursive_entries(&self.notes_root)?
            .into_iter()
            .filter(|entry| self.file_entry_matches_filter(entry, &query))
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        self.file_search_results = results;
        self.ensure_file_selection_visible();
        Ok(())
    }

    fn file_entry_matches_filter(&self, entry: &FileEntry, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }

        let relative = entry
            .path
            .strip_prefix(&self.notes_root)
            .unwrap_or(&entry.path)
            .display()
            .to_string()
            .to_lowercase();
        let name = entry.name.to_lowercase();
        let metadata = if entry.is_dir {
            FileMetadata::default()
        } else {
            self.file_metadata(&entry.path)
        };
        let title = metadata.title.unwrap_or_default().to_lowercase();
        let tags = metadata.tags.join(" ").to_lowercase();

        Self::filter_tokens(trimmed).into_iter().all(|token| {
            let (negated, token) = if let Some(token) = token.strip_prefix('-') {
                (true, token)
            } else {
                (false, token.as_str())
            };

            let token_lower = token.to_lowercase();
            let matches = if let Some(value) = token_lower.strip_prefix("title:") {
                !value.is_empty() && title.contains(value)
            } else if let Some(value) = token_lower.strip_prefix("tag:") {
                !value.is_empty()
                    && metadata
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(value))
            } else if let Some(value) = token_lower.strip_prefix("tags:") {
                !value.is_empty()
                    && metadata
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(value))
            } else if let Some(value) = token_lower.strip_prefix("path:") {
                !value.is_empty() && relative.contains(value)
            } else if let Some(value) = token_lower.strip_prefix("name:") {
                !value.is_empty() && name.contains(value)
            } else {
                fuzzy_matches(&relative, &token_lower)
                    || fuzzy_matches(&name, &token_lower)
                    || (!title.is_empty() && title.contains(&token_lower))
                    || (!tags.is_empty() && tags.contains(&token_lower))
            };

            if negated {
                !matches
            } else {
                matches
            }
        })
    }

    fn collect_recursive_entries(root: &Path) -> Result<Vec<FileEntry>, Box<dyn Error>> {
        let mut entries = Vec::new();
        Self::collect_recursive_entries_into(root, &mut entries)?;
        Ok(entries)
    }

    fn collect_recursive_entries_into(
        dir: &Path,
        entries: &mut Vec<FileEntry>,
    ) -> Result<(), Box<dyn Error>> {
        let mut children = fs::read_dir(dir)?
            .map(|entry| {
                let entry = entry?;
                let path = entry.path();
                let file_type = entry.file_type()?;
                let metadata = entry.metadata()?;
                Ok(FileEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path,
                    is_dir: file_type.is_dir(),
                    size_bytes: metadata.len(),
                    modified_at: metadata
                        .modified()
                        .ok()
                        .map(|time| chrono::DateTime::<chrono::Local>::from(time))
                        .map(|time| time.format("%Y-%m-%d %H:%M").to_string()),
                })
            })
            .collect::<Result<Vec<_>, io::Error>>()?;

        children.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });

        for child in children {
            let recurse_path = child.path.clone();
            let is_dir = child.is_dir;
            entries.push(child);
            if is_dir {
                Self::collect_recursive_entries_into(&recurse_path, entries)?;
            }
        }
        Ok(())
    }

    pub fn sync_file_preview(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(entry) = self.selected_file_entry().cloned() else {
            self.previewed_file_path = None;
            self.previewed_file_content.clear();
            self.preview_scroll = 0;
            return Ok(());
        };

        self.previewed_file_path = Some(entry.path.clone());
        self.preview_scroll = 0;
        if entry.is_dir {
            let child_count = fs::read_dir(&entry.path)?.count();
            self.previewed_file_content = format!(
                "Directory: {}\nItems: {}\n\nPress Enter to open this folder.",
                entry.path.display(),
                child_count
            );
        } else {
            let bytes = fs::read(&entry.path)?;
            let content = String::from_utf8_lossy(&bytes).into_owned();
            self.previewed_file_content = render_preview_content(&entry.path, &content);
        }
        Ok(())
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

fn render_preview_content(path: &Path, content: &str) -> String {
    let is_markdown = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false);

    if is_markdown {
        render_markdown_preview(content)
    } else {
        content.to_string()
    }
}

fn load_file_shortcuts(path: &Path) -> Result<Vec<SavedFileShortcut>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(serde_json::from_str(&content)?)
}

fn save_file_shortcuts(path: &Path, shortcuts: &[SavedFileShortcut]) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(shortcuts)?;
    fs::write(path, content)?;
    Ok(())
}

fn load_palette_history(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(serde_json::from_str(&content)?)
}

fn save_palette_history(path: &Path, commands: &[String]) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(commands)?;
    fs::write(path, content)?;
    Ok(())
}

fn load_custom_templates(dir: &Path) -> Result<Vec<TemplateDefinition>, Box<dyn Error>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut templates = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_file() {
            let content = fs::read_to_string(&path)?;
            let name = path
                .file_stem()
                .and_then(OsStr::to_str)
                .unwrap_or("template")
                .to_string();
            templates.push(TemplateDefinition {
                name,
                content,
                is_custom: true,
            });
        }
    }
    templates.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(templates)
}

fn copy_path_recursive(source: &Path, target: &Path) -> Result<(), Box<dyn Error>> {
    if source.is_dir() {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let child_source = entry.path();
            let child_target = target.join(entry.file_name());
            copy_path_recursive(&child_source, &child_target)?;
        }
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
    }
    Ok(())
}

fn parse_file_metadata(content: &str) -> FileMetadata {
    let mut metadata = FileMetadata::default();
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return metadata;
    }

    let frontmatter_lines = lines
        .by_ref()
        .take_while(|line| *line != "---")
        .collect::<Vec<_>>();

    let mut i = 0;
    while i < frontmatter_lines.len() {
        let line = frontmatter_lines[i].trim();
        if let Some(rest) = line.strip_prefix("title:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                metadata.title = Some(value.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("tags:") {
            let rest = rest.trim();
            if rest.starts_with('[') && rest.ends_with(']') {
                metadata.tags.extend(
                    rest.trim_matches(|c| c == '[' || c == ']')
                        .split(',')
                        .map(|tag| tag.trim().trim_matches('"').trim_matches('\''))
                        .filter(|tag| !tag.is_empty())
                        .map(|tag| tag.to_string()),
                );
            } else if !rest.is_empty() {
                metadata.tags.extend(
                    rest.split(',')
                        .map(|tag| tag.trim().trim_matches('"').trim_matches('\''))
                        .filter(|tag| !tag.is_empty())
                        .map(|tag| tag.to_string()),
                );
            } else {
                i += 1;
                while i < frontmatter_lines.len() {
                    let tag_line = frontmatter_lines[i].trim();
                    if let Some(tag) = tag_line.strip_prefix("- ") {
                        let tag = tag.trim().trim_matches('"').trim_matches('\'');
                        if !tag.is_empty() {
                            metadata.tags.push(tag.to_string());
                        }
                        i += 1;
                    } else {
                        i = i.saturating_sub(1);
                        break;
                    }
                }
            }
        }
        i += 1;
    }

    metadata.tags.sort();
    metadata.tags.dedup();
    metadata
}

fn line_count(content: &str) -> usize {
    if content.is_empty() {
        1
    } else {
        content.lines().count().max(1)
    }
}

fn extract_note_references(
    notes_root: &Path,
    source_path: &Path,
    content: &str,
) -> Vec<NoteReference> {
    let mut references = Vec::new();

    let mut remainder = content;
    while let Some(start) = remainder.find("[[") {
        let after_start = &remainder[start + 2..];
        if let Some(end) = after_start.find("]]") {
            let raw = after_start[..end].trim();
            if let Some(path) = resolve_reference_path(notes_root, source_path, raw) {
                references.push(NoteReference {
                    label: raw.to_string(),
                    path,
                });
            }
            remainder = &after_start[end + 2..];
        } else {
            break;
        }
    }

    let parser = Parser::new(content);
    for event in parser {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            let raw = dest_url.to_string();
            if let Some(path) = resolve_reference_path(notes_root, source_path, &raw) {
                references.push(NoteReference { label: raw, path });
            }
        }
    }

    references.sort_by(|left, right| left.label.cmp(&right.label));
    references.dedup_by(|left, right| left.path == right.path);
    references
}

fn resolve_reference_path(notes_root: &Path, source_path: &Path, raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with('#')
    {
        return None;
    }

    let candidate = if trimmed.starts_with('/') {
        notes_root.join(trimmed.trim_start_matches('/'))
    } else if trimmed.contains('/') || trimmed.ends_with(".md") || trimmed.ends_with(".markdown") {
        source_path.parent().unwrap_or(notes_root).join(trimmed)
    } else {
        notes_root.join(format!("{trimmed}.md"))
    };

    let normalized = normalize_note_path(&candidate);
    if normalized.starts_with(notes_root) && normalized.exists() {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_note_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

pub fn format_file_size(size_bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if size_bytes >= MB {
        format!("{:.1} MB", size_bytes as f64 / MB as f64)
    } else if size_bytes >= KB {
        format!("{:.1} KB", size_bytes as f64 / KB as f64)
    } else {
        format!("{size_bytes} B")
    }
}

fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }

    text.char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| text.len())
}

fn render_markdown_preview(content: &str) -> String {
    let mut output = String::new();
    let mut bullet_depth = 0usize;
    let mut current_heading: Option<HeadingLevel> = None;
    let mut in_code_block = false;

    for event in Parser::new(content) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                output.push('\n');
                current_heading = None;
            }
            Event::Start(Tag::List(_)) => {
                bullet_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                bullet_depth = bullet_depth.saturating_sub(1);
                output.push('\n');
            }
            Event::Start(Tag::Item) => {
                output.push_str(&"  ".repeat(bullet_depth.saturating_sub(1)));
                output.push_str("- ");
            }
            Event::End(TagEnd::Item) => output.push('\n'),
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                output.push_str("\n```text\n");
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                output.push_str("\n```\n");
            }
            Event::SoftBreak | Event::HardBreak => output.push('\n'),
            Event::Rule => output.push_str("\n----------------\n"),
            Event::Text(text) => {
                if let Some(level) = current_heading {
                    let prefix = match level {
                        HeadingLevel::H1 => "# ",
                        HeadingLevel::H2 => "## ",
                        HeadingLevel::H3 => "### ",
                        HeadingLevel::H4 => "#### ",
                        HeadingLevel::H5 => "##### ",
                        HeadingLevel::H6 => "###### ",
                    };
                    if output.is_empty() || output.ends_with('\n') {
                        output.push_str(prefix);
                    }
                }
                output.push_str(&text);
            }
            Event::Code(text) => {
                output.push('`');
                output.push_str(&text);
                output.push('`');
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                output.push('[');
                output.push_str(&dest_url);
                output.push_str("] ");
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                if in_code_block {
                    output.push_str(&html);
                }
            }
            _ => {}
        }
    }

    output.lines().take(200).collect::<Vec<_>>().join("\n")
}

fn fuzzy_matches(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let mut query_chars = query.chars();
    let mut current = query_chars.next();
    for ch in text.chars() {
        if let Some(expected) = current {
            if ch == expected {
                current = query_chars.next();
                if current.is_none() {
                    return true;
                }
            }
        } else {
            return true;
        }
    }
    current.is_none()
}

fn shell_quote(path: &Path) -> String {
    let escaped = path.display().to_string().replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::{parse_file_metadata, App, FileShortcutKind, InputMode, NotesView};
    use chrono::Local;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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

    fn temp_notes_root(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}_{}",
            prefix,
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(format!("task_manager_cli_notes_files_{unique}"))
    }

    fn test_app(db_path: &str, prefix: &str) -> Result<App, Box<dyn std::error::Error>> {
        App::new_with_notes_root(db_path, temp_notes_root(prefix))
    }

    #[test]
    fn begin_add_note_resets_inputs() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("begin_add");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "begin_add")?;

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
        let mut app = test_app(&db_path_str, "begin_edit")?;

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
        let mut app = test_app(&db_path_str, "blank_title")?;

        let error = app.add_note("   ", "Body").unwrap_err();
        assert!(error.to_string().contains("Note title cannot be empty"));

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn reset_inputs_clears_inline_feedback() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("feedback");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "feedback")?;

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
        let mut app = test_app(&db_path_str, "filter")?;

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
        let mut app = test_app(&db_path_str, "note_filter_tokens")?;

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
        let mut app = test_app(&db_path_str, "note_filter_phrases")?;

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
        let mut app = test_app(&db_path_str, "note_preset")?;

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

    #[test]
    fn file_browser_loads_directories_before_files() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_browser");
        let notes_root = temp_notes_root("file_browser");
        fs::create_dir_all(notes_root.join("projects"))?;
        fs::write(notes_root.join("inbox.md"), b"# Inbox")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

        assert_eq!(app.active_view, NotesView::Files);
        assert_eq!(app.file_entries.len(), 2);
        assert!(app.file_entries[0].is_dir);
        assert_eq!(app.file_entries[0].name, "projects");
        assert_eq!(app.file_entries[1].name, "inbox.md");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_browser_can_create_and_open_files() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("create_file");
        let notes_root = temp_notes_root("create_file");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_create_file();
        app.file_name_input = "daily".to_string();
        app.create_file()?;

        assert!(notes_root.join("daily.md").exists());
        assert_eq!(app.input_mode, InputMode::Normal);

        app.open_selected_file_entry()?;
        assert_eq!(app.input_mode, InputMode::ViewingFile);
        assert_eq!(
            app.viewed_file_path.as_deref(),
            Some(notes_root.join("daily.md").as_path())
        );

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn create_file_uses_selected_template() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_template");
        let notes_root = temp_notes_root("file_template");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_create_file();
        app.file_name_input = "standup".to_string();
        app.file_template_selected = 2;
        app.create_file()?;

        let content = fs::read_to_string(notes_root.join("standup.md"))?;
        assert!(content.contains("# Meeting: standup"));
        assert!(content.contains("## Agenda"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn daily_template_renders_date_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("template_render");
        let notes_root = temp_notes_root("template_render");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.file_template_selected = 1;
        let content = app.render_selected_template("daily.md");

        assert!(content.contains("## Goals"));
        assert!(content.contains(&Local::now().format("%Y-%m-%d").to_string()));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn custom_templates_load_from_notes_templates_dir() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("custom_templates");
        let notes_root = temp_notes_root("custom_templates");
        let templates_dir = notes_root
            .parent()
            .expect("temp notes root should have parent")
            .join("templates");
        fs::create_dir_all(&templates_dir)?;
        fs::write(
            templates_dir.join("brainstorm.md"),
            "# {{title}}\n\n## Ideas\n\n- \n",
        )?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_create_file();

        let templates = app.all_file_templates();
        assert!(templates
            .iter()
            .any(|template| template.name == "brainstorm"));

        app.file_template_selected = templates
            .iter()
            .position(|template| template.name == "brainstorm")
            .expect("custom template should be selectable");
        let content = app.render_selected_template("idea.md");
        assert!(content.contains("# idea"));
        assert!(content.contains("## Ideas"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        let _ = fs::remove_dir_all(templates_dir);
        Ok(())
    }

    #[test]
    fn file_browser_can_create_rename_and_delete_directories(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("dir_ops");
        let notes_root = temp_notes_root("dir_ops");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

        app.begin_create_directory();
        app.file_name_input = "projects".to_string();
        app.create_directory()?;
        assert!(notes_root.join("projects").exists());

        app.begin_rename_selected_entry();
        app.file_name_input = "archive".to_string();
        app.rename_selected_entry()?;
        assert!(notes_root.join("archive").exists());
        assert!(!notes_root.join("projects").exists());

        app.begin_delete_selected_entry();
        app.delete_selected_entry()?;
        assert!(!notes_root.join("archive").exists());

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_search_finds_nested_entries() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_search");
        let notes_root = temp_notes_root("file_search");
        fs::create_dir_all(notes_root.join("projects/alpha"))?;
        fs::write(notes_root.join("projects/alpha/roadmap.md"), b"# Roadmap")?;
        fs::write(notes_root.join("scratch.md"), b"scratch")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_file_search();
        for ch in "rdmp".chars() {
            app.append_file_search_char(ch);
        }

        assert!(app.has_file_search());
        assert_eq!(app.file_search_results.len(), 1);
        assert_eq!(app.file_search_results[0].name, "roadmap.md");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn parse_file_metadata_extracts_title_and_tags() {
        let metadata = parse_file_metadata(
            "---\n\
title: Sprint Review\n\
tags:\n\
- work\n\
- weekly\n\
---\n\
# Body\n",
        );

        assert_eq!(metadata.title.as_deref(), Some("Sprint Review"));
        assert_eq!(
            metadata.tags,
            vec!["weekly".to_string(), "work".to_string()]
        );
    }

    #[test]
    fn file_search_supports_frontmatter_title_and_tag_tokens(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_search_metadata");
        let notes_root = temp_notes_root("file_search_metadata");
        fs::create_dir_all(notes_root.join("projects"))?;
        fs::write(
            notes_root.join("projects/review.md"),
            b"---\ntitle: Sprint Review\ntags: [work, planning]\n---\n# Review\n",
        )?;
        fs::write(
            notes_root.join("personal.md"),
            b"---\ntitle: Weekend Plans\ntags: [personal]\n---\n# Plans\n",
        )?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

        app.set_file_search_query("tag:work")?;
        assert_eq!(app.file_search_results.len(), 1);
        assert_eq!(app.file_search_results[0].name, "review.md");

        app.set_file_search_query("title:\"Sprint Review\" -tag:personal")?;
        assert_eq!(app.file_search_results.len(), 1);
        assert_eq!(app.file_search_results[0].name, "review.md");

        app.set_file_search_query("path:projects")?;
        assert!(app
            .file_search_results
            .iter()
            .any(|entry| entry.name == "review.md"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn markdown_files_render_as_terminal_preview() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("markdown_preview");
        let notes_root = temp_notes_root("markdown_preview");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("roadmap.md");
        fs::write(&file_path, b"# Roadmap\n\n- Alpha\n- Beta\n")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);

        assert!(app.previewed_file_content.contains("# Roadmap"));
        assert!(app.previewed_file_content.contains("- Alpha"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn inline_file_edit_saves_updated_content() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("inline_edit");
        let notes_root = temp_notes_root("inline_edit");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("draft.md");
        fs::write(&file_path, b"# Draft\n")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.begin_inline_file_edit()?;
        app.file_edit_content = "# Updated\nBody".to_string();
        app.save_inline_file_edit()?;

        assert_eq!(fs::read_to_string(&file_path)?, "# Updated\nBody");
        assert_eq!(app.input_mode, InputMode::ViewingFile);
        assert!(app.previewed_file_content.contains("# Updated"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn inline_editor_supports_cursor_navigation_and_insertions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("inline_cursor");
        let notes_root = temp_notes_root("inline_cursor");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("draft.md");
        fs::write(&file_path, b"ab\ncd")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.begin_inline_file_edit()?;

        app.move_file_edit_up();
        app.move_file_edit_left();
        app.insert_file_edit_char('X');
        app.insert_file_edit_newline();
        app.insert_file_edit_char('Y');

        assert_eq!(app.file_edit_content, "aX\nYb\ncd");
        assert_eq!(app.file_edit_cursor_row, 1);
        assert_eq!(app.file_edit_cursor_col, 1);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn inline_editor_backspace_merges_lines() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("inline_backspace");
        let notes_root = temp_notes_root("inline_backspace");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("draft.md");
        fs::write(&file_path, b"ab\ncd")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.begin_inline_file_edit()?;

        app.move_file_edit_up();
        app.move_file_edit_down();
        app.file_edit_cursor_col = 0;
        app.backspace_file_edit();

        assert_eq!(app.file_edit_content, "abcd");
        assert_eq!(app.file_edit_cursor_row, 0);
        assert_eq!(app.file_edit_cursor_col, 2);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn inline_editor_tracks_preferred_column_and_horizontal_scroll(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("inline_scroll");
        let notes_root = temp_notes_root("inline_scroll");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("draft.md");
        fs::write(&file_path, b"abcdefghij\nxy")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.begin_inline_file_edit()?;

        for _ in 0..5 {
            app.move_file_edit_left();
        }
        app.ensure_file_edit_cursor_visible(4, 3);
        assert_eq!(app.file_edit_scroll_x, 6);

        app.move_file_edit_down();
        assert_eq!(app.file_edit_cursor_col, 2);
        app.move_file_edit_up();
        assert_eq!(app.file_edit_cursor_col, 8);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_entries_include_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_metadata");
        let notes_root = temp_notes_root("file_metadata");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("info.md");
        fs::write(&file_path, b"12345")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        let entry = app
            .file_entries
            .iter()
            .find(|entry| entry.path == file_path)
            .expect("file entry should exist");

        assert_eq!(entry.size_bytes, 5);
        assert!(entry.modified_at.is_some());

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_shortcuts_can_pin_dirs_and_save_searches() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_shortcuts");
        let notes_root = temp_notes_root("file_shortcuts");
        fs::create_dir_all(notes_root.join("projects"))?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.current_dir = notes_root.join("projects");
        app.toggle_pin_current_directory()?;
        app.set_file_search_query("roadmap")?;
        app.save_current_file_search()?;

        assert_eq!(app.file_shortcuts.len(), 2);
        assert_eq!(app.file_shortcuts[0].kind, FileShortcutKind::Directory);
        assert_eq!(app.file_shortcuts[1].kind, FileShortcutKind::Search);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_entries_can_be_moved_to_new_paths() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("move_entry");
        let notes_root = temp_notes_root("move_entry");
        fs::create_dir_all(notes_root.join("inbox"))?;
        fs::create_dir_all(notes_root.join("archive"))?;
        let source = notes_root.join("inbox/note.md");
        fs::write(&source, b"hello")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.current_dir = notes_root.join("inbox");
        app.load_file_entries()?;
        app.select_file_entry_path(&source);
        app.begin_move_selected_entry();
        app.file_name_input = "archive/note.md".to_string();
        app.move_selected_entry()?;

        assert!(!source.exists());
        assert!(notes_root.join("archive/note.md").exists());

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn directories_can_be_copied_recursively() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("copy_entry");
        let notes_root = temp_notes_root("copy_entry");
        fs::create_dir_all(notes_root.join("projects/alpha"))?;
        fs::write(notes_root.join("projects/alpha/roadmap.md"), b"roadmap")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.current_dir = notes_root.join("projects");
        app.load_file_entries()?;
        app.select_file_entry_path(&notes_root.join("projects/alpha"));
        app.begin_copy_selected_entry();
        app.file_name_input = "archive/alpha-copy".to_string();
        app.copy_selected_entry()?;

        assert!(notes_root.join("projects/alpha/roadmap.md").exists());
        assert!(notes_root.join("archive/alpha-copy/roadmap.md").exists());

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn note_references_detect_wiki_and_markdown_links() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("note_refs");
        let notes_root = temp_notes_root("note_refs");
        fs::create_dir_all(notes_root.join("projects"))?;
        let target = notes_root.join("projects/roadmap.md");
        fs::write(&target, b"# Roadmap")?;
        let source = notes_root.join("index.md");
        fs::write(
            &source,
            b"See [[projects/roadmap.md]] and [Roadmap](projects/roadmap.md).",
        )?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        let refs = app.file_references(&source);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].path, target);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn backlink_discovery_finds_reverse_links() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("backlinks");
        let notes_root = temp_notes_root("backlinks");
        fs::create_dir_all(notes_root.join("topics"))?;
        let target = notes_root.join("topics/alpha.md");
        fs::write(&target, b"# Alpha")?;
        fs::write(notes_root.join("index.md"), b"[[topics/alpha.md]]")?;
        fs::write(notes_root.join("journal.md"), b"[Alpha](topics/alpha.md)")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        let backlinks = app.file_backlinks(&target);

        assert_eq!(backlinks.len(), 2);
        assert_eq!(backlinks[0].label, "/index.md");
        assert_eq!(backlinks[1].label, "/journal.md");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn related_links_can_be_opened_from_current_note() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("open_related");
        let notes_root = temp_notes_root("open_related");
        fs::create_dir_all(notes_root.join("topics"))?;
        let target = notes_root.join("topics/alpha.md");
        fs::write(&target, b"# Alpha")?;
        let source = notes_root.join("index.md");
        fs::write(&source, b"[[topics/alpha.md]]")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.open_file_path(&source)?;
        app.begin_file_links();
        app.open_selected_related_link()?;

        assert_eq!(app.viewed_file_path.as_deref(), Some(target.as_path()));
        assert_eq!(app.input_mode, InputMode::ViewingFile);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn create_or_open_daily_note_uses_daily_template() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("daily_shortcut");
        let notes_root = temp_notes_root("daily_shortcut");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.create_or_open_daily_note()?;

        let expected = notes_root
            .join("daily")
            .join(format!("{}.md", Local::now().format("%Y-%m-%d")));
        assert_eq!(app.viewed_file_path.as_deref(), Some(expected.as_path()));
        assert!(expected.exists());
        assert!(fs::read_to_string(&expected)?.contains("## Goals"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn external_editor_updates_preview_and_view_content() -> Result<(), Box<dyn std::error::Error>>
    {
        let db_path = temp_db_path("editor");
        let notes_root = temp_notes_root("editor");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("edit.md");
        fs::write(&file_path, b"before")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.editor_command = Some("printf 'after' >".to_string() + " ");
        app.select_entry_path(&file_path);
        app.edit_selected_file_in_editor()?;

        assert_eq!(fs::read_to_string(&file_path)?, "after");
        assert_eq!(app.previewed_file_content, "after");
        assert_eq!(app.viewed_file_content, "after");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn opening_or_reselecting_files_resets_scroll_offsets() -> Result<(), Box<dyn std::error::Error>>
    {
        let db_path = temp_db_path("scroll_reset");
        let notes_root = temp_notes_root("scroll_reset");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("long.md");
        fs::write(&file_path, b"one\ntwo\nthree\nfour\nfive\nsix\n")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.scroll_preview_down(3);
        app.open_file_path(&file_path)?;
        app.scroll_viewed_file_down(4);
        assert_eq!(app.preview_scroll, 0);
        assert_eq!(app.viewed_file_scroll, 4);

        app.select_file_entry_path(&file_path);
        assert_eq!(app.preview_scroll, 0);
        app.open_file_path(&file_path)?;
        assert_eq!(app.viewed_file_scroll, 0);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn command_palette_round_trips_mode_and_query() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("command_palette");
        let notes_root = temp_notes_root("command_palette");
        fs::create_dir_all(&notes_root)?;
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

        app.begin_file_search();
        app.command_palette_query = "stale".to_string();
        app.begin_command_palette();

        assert_eq!(app.input_mode, InputMode::CommandPalette);
        assert_eq!(app.command_palette_return_mode, InputMode::SearchingFiles);
        assert!(app.command_palette_query.is_empty());

        app.close_command_palette();
        assert_eq!(app.input_mode, InputMode::SearchingFiles);

        app.record_palette_command("search_files")?;
        app.record_palette_command("help")?;
        app.record_palette_command("search_files")?;
        assert_eq!(app.recent_palette_commands[0], "search_files");
        assert_eq!(app.recent_palette_commands[1], "help");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }
}
