use chrono::Local;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

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
    Editing,
    DeleteConfirm,
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
    pub editing_index: Option<usize>,
    pub form_values: Vec<String>,
    pub editing_field: usize,
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
                    "Next 1:1",
                    "Agenda",
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
                field_labels: &["Task", "Owner", "Status", "Due Date", "Context"],
            },
            ToolKind::Decision => ToolSpec {
                kind: self,
                title: "Decision Log",
                accent: Accent::Decisions,
                list_title: "Decisions",
                detail_title: "Decision Detail",
                store_filename: "decisions.json",
                field_labels: &["Decision", "Owner", "Status", "Date", "Rationale", "Impact"],
            },
        }
    }
}

impl App {
    pub fn new(kind: ToolKind) -> Result<Self, Box<dyn Error>> {
        let spec = kind.spec();
        let store_path = store_path_for(kind);
        if let Some(parent) = store_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let records = load_records(&store_path)?;
        let record_count = records.len();

        Ok(Self {
            spec,
            store_path,
            records,
            selected: 0,
            input_mode: InputMode::Normal,
            editing_index: None,
            form_values: spec.default_values(),
            editing_field: 0,
            feedback: None,
            logs: vec![format!("Loaded {} records.", record_count)],
        })
    }

    pub fn selected_record(&self) -> Option<&ToolRecord> {
        self.records.get(self.selected)
    }

    pub fn move_selection_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_selection_down(&mut self) {
        if self.selected + 1 < self.records.len() {
            self.selected += 1;
        }
    }

    pub fn begin_add(&mut self) {
        self.input_mode = InputMode::Editing;
        self.editing_index = None;
        self.form_values = self.spec.default_values();
        self.editing_field = 0;
        self.feedback = None;
    }

    pub fn begin_edit(&mut self) {
        if let Some(values) = self
            .selected_record()
            .map(|record| self.spec.normalize_values(&record.values))
        {
            self.input_mode = InputMode::Editing;
            self.editing_index = Some(self.selected);
            self.form_values = values;
            self.editing_field = 0;
            self.feedback = None;
        }
    }

    pub fn begin_delete(&mut self) {
        if self.selected_record().is_some() {
            self.input_mode = InputMode::DeleteConfirm;
            self.feedback = None;
        }
    }

    pub fn cancel_modal(&mut self) {
        self.input_mode = InputMode::Normal;
        self.editing_index = None;
        self.feedback = None;
    }

    pub fn focus_next_field(&mut self) {
        self.editing_field = (self.editing_field + 1) % self.spec.field_labels.len();
        self.feedback = None;
    }

    pub fn focus_prev_field(&mut self) {
        if self.editing_field == 0 {
            self.editing_field = self.spec.field_labels.len().saturating_sub(1);
        } else {
            self.editing_field -= 1;
        }
        self.feedback = None;
    }

    pub fn append_char(&mut self, c: char) {
        if let Some(value) = self.form_values.get_mut(self.editing_field) {
            value.push(c);
        }
        self.feedback = None;
    }

    pub fn backspace(&mut self) {
        if let Some(value) = self.form_values.get_mut(self.editing_field) {
            value.pop();
        }
        self.feedback = None;
    }

    pub fn advance_or_save(&mut self) -> Result<(), Box<dyn Error>> {
        if self.editing_field + 1 < self.spec.field_labels.len() {
            self.focus_next_field();
            return Ok(());
        }
        self.save_form()
    }

    pub fn save_form(&mut self) -> Result<(), Box<dyn Error>> {
        if self
            .form_values
            .first()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
        {
            self.feedback = Some(format!("{} cannot be empty.", self.spec.field_labels[0]));
            return Ok(());
        }

        let record = ToolRecord {
            id: self
                .editing_index
                .and_then(|index| self.records.get(index).map(|record| record.id))
                .unwrap_or_else(|| next_id(&self.records)),
            values: self.form_values.clone(),
            updated_at: timestamp(),
        };

        if let Some(index) = self.editing_index {
            self.records[index] = record;
            self.log("Updated record.");
        } else {
            self.records.insert(0, record);
            self.selected = 0;
            self.log("Added record.");
        }

        self.persist()?;
        self.input_mode = InputMode::Normal;
        self.editing_index = None;
        self.feedback = Some("Saved.".to_string());
        Ok(())
    }

    pub fn delete_selected(&mut self) -> Result<(), Box<dyn Error>> {
        if self.selected < self.records.len() {
            self.records.remove(self.selected);
            if self.selected >= self.records.len() && self.selected > 0 {
                self.selected -= 1;
            }
            self.persist()?;
            self.log("Deleted record.");
        }
        self.input_mode = InputMode::Normal;
        self.feedback = None;
        Ok(())
    }

    pub fn cycle_status(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(status_index) = self.spec.status_field_index() else {
            return Ok(());
        };
        let Some(statuses) = self.spec.statuses() else {
            return Ok(());
        };
        let Some(record) = self.records.get_mut(self.selected) else {
            return Ok(());
        };
        let values = self.spec.normalize_values(&record.values);
        let current = values
            .get(status_index)
            .map(|value| value.as_str())
            .unwrap_or_default();
        let current_index = statuses
            .iter()
            .position(|status| *status == current)
            .unwrap_or(0);
        let next_status = statuses[(current_index + 1) % statuses.len()];
        let mut normalized = values;
        normalized[status_index] = next_status.to_string();
        record.values = normalized;
        record.updated_at = timestamp();
        self.persist()?;
        self.log(&format!("Status set to {next_status}."));
        Ok(())
    }

    pub fn list_items(&self) -> Vec<(String, String)> {
        self.records
            .iter()
            .map(|record| {
                let values = self.spec.normalize_values(&record.values);
                let primary = values.first().cloned().unwrap_or_default();
                let secondary = self.spec.list_summary(&values);
                (primary, secondary)
            })
            .collect()
    }

    pub fn detail_lines(&self) -> Vec<String> {
        let Some(record) = self.selected_record() else {
            return vec!["No record selected.".to_string()];
        };
        let values = self.spec.normalize_values(&record.values);
        let mut lines = self
            .spec
            .field_labels
            .iter()
            .enumerate()
            .map(|(index, label)| format!("{label}: {}", values[index]))
            .collect::<Vec<_>>();
        lines.push(format!("Updated: {}", record.updated_at));
        lines
    }

    pub fn draft_lines(&self) -> Vec<String> {
        self.spec
            .field_labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let marker = if index == self.editing_field {
                    ">"
                } else {
                    " "
                };
                format!("{marker} {label}: {}", self.form_values[index])
            })
            .collect()
    }

    fn persist(&self) -> Result<(), Box<dyn Error>> {
        let content = serde_json::to_string_pretty(&self.records)?;
        fs::write(&self.store_path, content)?;
        Ok(())
    }

    fn log(&mut self, message: &str) {
        self.logs.insert(
            0,
            format!("{} | {}", Local::now().format("%Y-%m-%d %H:%M:%S"), message),
        );
        self.logs.truncate(8);
    }
}

impl ToolSpec {
    pub fn default_values(&self) -> Vec<String> {
        match self.kind {
            ToolKind::OneOnOne => vec![
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
            ToolKind::Delegation => vec![
                String::new(),
                String::new(),
                "Delegated".to_string(),
                String::new(),
                String::new(),
            ],
            ToolKind::Decision => vec![
                String::new(),
                String::new(),
                "Proposed".to_string(),
                Local::now().format("%Y-%m-%d").to_string(),
                String::new(),
                String::new(),
            ],
        }
    }

    pub fn normalize_values(&self, values: &[String]) -> Vec<String> {
        let mut normalized = self.default_values();
        for (index, value) in values.iter().enumerate().take(normalized.len()) {
            normalized[index] = value.clone();
        }
        normalized
    }

    pub fn list_summary(&self, values: &[String]) -> String {
        match self.kind {
            ToolKind::OneOnOne => {
                format!(
                    "next {} | follow-ups {}",
                    blank_dash(&values[1]),
                    blank_dash(&values[3])
                )
            }
            ToolKind::Delegation => format!(
                "{} | {}",
                blank_dash(&values[2]),
                with_prefix("due ", &values[3])
            ),
            ToolKind::Decision => {
                format!("{} | {}", blank_dash(&values[2]), blank_dash(&values[3]))
            }
        }
    }

    pub fn status_field_index(&self) -> Option<usize> {
        match self.kind {
            ToolKind::OneOnOne => None,
            ToolKind::Delegation => Some(2),
            ToolKind::Decision => Some(2),
        }
    }

    pub fn statuses(&self) -> Option<&'static [&'static str]> {
        match self.kind {
            ToolKind::OneOnOne => None,
            ToolKind::Delegation => Some(&["Delegated", "In Progress", "Blocked", "Done"]),
            ToolKind::Decision => Some(&["Proposed", "Decided", "Superseded"]),
        }
    }
}

pub fn load_dashboard_snapshot(kind: ToolKind) -> Result<DashboardSnapshot, Box<dyn Error>> {
    let spec = kind.spec();
    let store_path = store_path_for(kind);
    let records = load_records(&store_path)?;

    let (stat_a_value, stat_b_value, stat_a_label, stat_b_label) = match kind {
        ToolKind::OneOnOne => (
            records
                .iter()
                .filter(|record| value_at(record, 1).trim().is_empty().not())
                .count(),
            records
                .iter()
                .filter(|record| value_at(record, 3).trim().is_empty().not())
                .count(),
            "scheduled",
            "follow-ups",
        ),
        ToolKind::Delegation => (
            records
                .iter()
                .filter(|record| value_at(record, 2) != "Done")
                .count(),
            records
                .iter()
                .filter(|record| value_at(record, 2) == "Blocked")
                .count(),
            "open",
            "blocked",
        ),
        ToolKind::Decision => (
            records
                .iter()
                .filter(|record| value_at(record, 2) == "Decided")
                .count(),
            records
                .iter()
                .filter(|record| value_at(record, 2) == "Proposed")
                .count(),
            "decided",
            "proposed",
        ),
    };

    Ok(DashboardSnapshot {
        store_path,
        count: records.len(),
        recent_items: records
            .iter()
            .take(5)
            .map(|record| spec.list_summary(&spec.normalize_values(&record.values)))
            .zip(
                records
                    .iter()
                    .take(5)
                    .map(|record| value_at(record, 0).to_string()),
            )
            .map(|(summary, title)| format!("{title} | {summary}"))
            .collect(),
        stat_a_label,
        stat_a_value,
        stat_b_label,
        stat_b_value,
    })
}

fn store_path_for(kind: ToolKind) -> PathBuf {
    let base = std::env::var("LEADERSHIP_TOOLS_DIR").unwrap_or_else(|_| ".leadership".to_string());
    PathBuf::from(base).join(kind.spec().store_filename)
}

fn load_records(path: &Path) -> Result<Vec<ToolRecord>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&content)?)
}

fn next_id(records: &[ToolRecord]) -> u64 {
    records.iter().map(|record| record.id).max().unwrap_or(0) + 1
}

fn timestamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn blank_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "-"
    } else {
        value
    }
}

fn with_prefix(prefix: &str, value: &str) -> String {
    if value.trim().is_empty() {
        "-".to_string()
    } else {
        format!("{prefix}{value}")
    }
}

fn value_at(record: &ToolRecord, index: usize) -> &str {
    record
        .values
        .get(index)
        .map(|value| value.as_str())
        .unwrap_or("")
}

trait BoolExt {
    fn not(self) -> bool;
}

impl BoolExt for bool {
    fn not(self) -> bool {
        !self
    }
}
