use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::db::notes::models::Note;
use crate::db::notes::operations::DbOperations;
use crate::filter_presets::SavedFilterPreset;

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
