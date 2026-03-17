use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::ui_style::Accent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    OneOnOne,
    Delegation,
    Decision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Filtering,
    Editing,
    DeleteConfirm,
    LinkedRecords,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkedRecordKind {
    Task,
    Note,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedRecord {
    pub kind: LinkedRecordKind,
    pub id: i32,
    pub title: String,
    pub summary: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ToolRecord {
    pub id: u64,
    pub values: Vec<String>,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct DashboardSnapshot {
    pub store_path: PathBuf,
    pub count: usize,
    pub recent_items: Vec<String>,
    pub stat_a_label: &'static str,
    pub stat_a_value: usize,
    pub stat_b_label: &'static str,
    pub stat_b_value: usize,
}

impl Default for DashboardSnapshot {
    fn default() -> Self {
        Self {
            store_path: PathBuf::new(),
            count: 0,
            recent_items: Vec::new(),
            stat_a_label: "",
            stat_a_value: 0,
            stat_b_label: "",
            stat_b_value: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ToolSpec {
    pub kind: ToolKind,
    pub title: &'static str,
    pub accent: Accent,
    pub list_title: &'static str,
    pub detail_title: &'static str,
    pub store_filename: &'static str,
    pub field_labels: &'static [&'static str],
}

pub struct App {
    pub spec: ToolSpec,
    pub store_path: PathBuf,
    pub records: Vec<ToolRecord>,
    pub selected: usize,
    pub input_mode: InputMode,
    pub filter_query: String,
    pub editing_index: Option<usize>,
    pub form_values: Vec<String>,
    pub field_touched: Vec<bool>,
    pub editing_field: usize,
    pub linked_record_selected: usize,
    pub linked_records: Vec<LinkedRecord>,
    pub feedback: Option<String>,
    pub logs: Vec<String>,
}

impl ToolKind {
    pub fn spec(self) -> ToolSpec {
        match self {
            ToolKind::OneOnOne => ToolSpec {
                kind: self,
                title: "1:1 Manager",
                accent: Accent::LeadershipPeople,
                list_title: "1:1s",
                detail_title: "1:1 Detail",
                store_filename: "one_on_ones.json",
                field_labels: &[
                    "Person",
                    "Relationship",
                    "Meeting Type",
                    "Team / Org",
                    "Manager / Chain",
                    "Next 1:1",
                    "Cadence",
                    "Last 1:1",
                    "Purpose",
                    "Agenda",
                    "Action Items",
                    "Follow-Ups",
                    "Private Notes",
                ],
            },
            ToolKind::Delegation => ToolSpec {
                kind: self,
                title: "Delegation Tracker",
                accent: Accent::Delegation,
                list_title: "Delegations",
                detail_title: "Delegation Detail",
                store_filename: "delegations.json",
                field_labels: &[
                    "Task",
                    "Owner",
                    "Status",
                    "Due Date",
                    "Next Follow-Up",
                    "Last Reminder",
                    "Context",
                ],
            },
            ToolKind::Decision => ToolSpec {
                kind: self,
                title: "Decision Log",
                accent: Accent::Decisions,
                list_title: "Decisions",
                detail_title: "Decision Detail",
                store_filename: "decisions.json",
                field_labels: &[
                    "Decision",
                    "Owner",
                    "Status",
                    "Date",
                    "Tags",
                    "Linked Notes",
                    "Linked Tasks",
                    "Rationale",
                    "Impact",
                    "Review Date",
                ],
            },
        }
    }
}
