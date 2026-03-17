use std::{collections::HashSet, path::PathBuf};

use crate::db::task_manager::models::{Task, Topic};
use crate::db::task_manager::operations::DbOperations;
use crate::filter_presets::SavedFilterPreset;

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
}

/// The overall application state.
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
    /// Current tab in special tasks popup. `0 = favourites`, `1 = completed`.
    pub special_tab_selected: usize,
    /// Selected task in special popup.
    pub special_task_selected: usize,
    /// Filter query for special popup tasks.
    pub special_task_filter: String,
    /// Cached favourites tasks.
    pub favourites_tasks: Vec<Task>,
    /// Cached completed tasks.
    pub completed_tasks: Vec<Task>,
}
