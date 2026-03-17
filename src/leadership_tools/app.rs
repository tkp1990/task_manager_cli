use chrono::{Duration, Local, NaiveDate};
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
                    "Next 1:1",
                    "Cadence",
                    "Last 1:1",
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
            filter_query: String::new(),
            editing_index: None,
            form_values: spec.default_values(),
            field_touched: vec![true; spec.field_labels.len()],
            editing_field: 0,
            linked_record_selected: 0,
            linked_records: Vec::new(),
            feedback: None,
            logs: vec![format!("Loaded {} records.", record_count)],
        })
    }

    pub fn selected_record(&self) -> Option<&ToolRecord> {
        self.records.get(self.selected)
    }

    pub fn move_selection_up(&mut self) {
        let indices = self.filtered_indices();
        if let Some(position) = indices.iter().position(|index| *index == self.selected) {
            if position > 0 {
                self.selected = indices[position - 1];
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        let indices = self.filtered_indices();
        if let Some(position) = indices.iter().position(|index| *index == self.selected) {
            if position + 1 < indices.len() {
                self.selected = indices[position + 1];
            }
        } else if let Some(first) = indices.first() {
            self.selected = *first;
        }
    }

    pub fn begin_filter(&mut self) {
        self.input_mode = InputMode::Filtering;
        self.feedback = None;
    }

    pub fn clear_filter(&mut self) {
        self.filter_query.clear();
        self.ensure_selected_visible();
        self.input_mode = InputMode::Normal;
    }

    pub fn keep_filter(&mut self) {
        self.ensure_selected_visible();
        self.input_mode = InputMode::Normal;
    }

    pub fn begin_add(&mut self) {
        self.input_mode = InputMode::Editing;
        self.editing_index = None;
        self.form_values = self.spec.default_values();
        self.field_touched = vec![false; self.spec.field_labels.len()];
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
            self.field_touched = vec![true; self.spec.field_labels.len()];
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

    pub fn begin_link_browser(&mut self) -> Result<(), Box<dyn Error>> {
        self.linked_records = self.resolve_linked_records()?;
        if self.linked_records.is_empty() {
            self.feedback = Some("No linked tasks or notes on this record.".to_string());
            return Ok(());
        }

        self.linked_record_selected = 0;
        self.input_mode = InputMode::LinkedRecords;
        self.feedback = None;
        Ok(())
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

    pub fn move_link_selection_down(&mut self) {
        if self.linked_record_selected + 1 < self.linked_records.len() {
            self.linked_record_selected += 1;
        }
    }

    pub fn move_link_selection_up(&mut self) {
        if self.linked_record_selected > 0 {
            self.linked_record_selected -= 1;
        }
    }

    pub fn selected_linked_record(&self) -> Option<&LinkedRecord> {
        self.linked_records.get(self.linked_record_selected)
    }

    pub fn append_char(&mut self, c: char) {
        match self.input_mode {
            InputMode::Filtering => {
                self.filter_query.push(c);
                self.ensure_selected_visible();
            }
            InputMode::Editing => {
                if let Some(value) = self.form_values.get_mut(self.editing_field) {
                    if !self
                        .field_touched
                        .get(self.editing_field)
                        .copied()
                        .unwrap_or(true)
                    {
                        value.clear();
                        if let Some(touched) = self.field_touched.get_mut(self.editing_field) {
                            *touched = true;
                        }
                    }
                    value.push(c);
                }
            }
            _ => {}
        }
        self.feedback = None;
    }

    pub fn backspace(&mut self) {
        match self.input_mode {
            InputMode::Filtering => {
                self.filter_query.pop();
                self.ensure_selected_visible();
            }
            InputMode::Editing => {
                if let Some(value) = self.form_values.get_mut(self.editing_field) {
                    if !self
                        .field_touched
                        .get(self.editing_field)
                        .copied()
                        .unwrap_or(true)
                    {
                        value.clear();
                        if let Some(touched) = self.field_touched.get_mut(self.editing_field) {
                            *touched = true;
                        }
                    }
                    value.pop();
                }
            }
            _ => {}
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
        self.ensure_selected_visible();
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

    pub fn quick_action(&mut self) -> Result<(), Box<dyn Error>> {
        match self.spec.kind {
            ToolKind::OneOnOne => self.bump_next_meeting(),
            ToolKind::Delegation | ToolKind::Decision => self.cycle_status(),
        }
    }

    pub fn send_delegation_reminder(&mut self) -> Result<(), Box<dyn Error>> {
        if !matches!(self.spec.kind, ToolKind::Delegation) {
            return Ok(());
        }

        let Some(record) = self.records.get_mut(self.selected) else {
            return Ok(());
        };

        let mut values = self.spec.normalize_values(&record.values);
        if values[2] == "Done" {
            self.feedback = Some("Completed delegations do not need reminders.".to_string());
            return Ok(());
        }

        let today = Local::now().date_naive();
        let follow_up_date = parse_ymd(&values[4]).unwrap_or(today) + Duration::days(3);
        values[4] = follow_up_date.format("%Y-%m-%d").to_string();
        values[5] = today.format("%Y-%m-%d").to_string();
        record.values = values;
        record.updated_at = timestamp();
        self.persist()?;
        self.feedback = Some("Reminder logged and follow-up advanced by 3 days.".to_string());
        self.log("Delegation reminder logged.");
        Ok(())
    }

    pub fn schedule_decision_review(&mut self) -> Result<(), Box<dyn Error>> {
        if !matches!(self.spec.kind, ToolKind::Decision) {
            return Ok(());
        }

        let Some(record) = self.records.get_mut(self.selected) else {
            return Ok(());
        };

        let mut values = self.spec.normalize_values(&record.values);
        let base_date = parse_ymd(&values[9])
            .or_else(|| parse_ymd(&values[3]))
            .unwrap_or_else(|| Local::now().date_naive());
        let next_review = base_date + Duration::days(14);
        values[9] = next_review.format("%Y-%m-%d").to_string();
        record.values = values;
        record.updated_at = timestamp();
        self.persist()?;
        self.feedback = Some("Decision review date moved forward by 14 days.".to_string());
        self.log("Decision review date updated.");
        Ok(())
    }

    pub fn complete_one_on_one(&mut self) -> Result<(), Box<dyn Error>> {
        if !matches!(self.spec.kind, ToolKind::OneOnOne) {
            return Ok(());
        }

        let Some(record) = self.records.get_mut(self.selected) else {
            return Ok(());
        };

        let mut values = self.spec.normalize_values(&record.values);
        let meeting_date = parse_ymd(&values[1]).unwrap_or_else(|| Local::now().date_naive());
        let cadence = cadence_duration(&values[2]);
        let next_date = meeting_date + cadence;

        values[3] = meeting_date.format("%Y-%m-%d").to_string();
        values[1] = next_date.format("%Y-%m-%d").to_string();
        values[5] = merge_rollover_items(&values[5], &values[4]);
        values[4].clear();

        record.values = values;
        record.updated_at = timestamp();
        self.persist()?;
        self.log("Completed 1:1 and advanced the next meeting.");
        Ok(())
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        self.records
            .iter()
            .enumerate()
            .filter(|(_, record)| self.spec.matches_filter(record, &self.filter_query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn has_filter(&self) -> bool {
        !self.filter_query.trim().is_empty()
    }

    pub fn ensure_selected_visible(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            self.selected = 0;
        } else if !indices.contains(&self.selected) {
            self.selected = indices[0];
        }
    }

    pub fn detail_lines(&self) -> Vec<String> {
        let Some(record) = self.selected_record() else {
            return vec!["No record selected.".to_string()];
        };
        let values = self.spec.normalize_values(&record.values);
        if matches!(self.spec.kind, ToolKind::OneOnOne) {
            let mut lines = vec![
                format!("Person: {}", values[0]),
                format!("Next 1:1: {}", blank_dash(&values[1])),
                format!("Cadence: {}", blank_dash(&values[2])),
                format!("Last 1:1: {}", blank_dash(&values[3])),
                format!("Agenda: {}", blank_dash(&values[4])),
                format!("Follow-Ups: {}", blank_dash(&values[5])),
                format!(
                    "Agenda Ready: {}",
                    if values[4].trim().is_empty() {
                        "No"
                    } else {
                        "Yes"
                    }
                ),
                format!("Private Notes: {}", blank_dash(&values[6])),
                format!("Updated: {}", record.updated_at),
            ];
            if let Some(next_date) = parse_ymd(&values[1]) {
                let days_until = (next_date - Local::now().date_naive()).num_days();
                lines.insert(2, format!("Days Until Next: {days_until}"));
            }
            return lines;
        }
        if matches!(self.spec.kind, ToolKind::Delegation) {
            let mut lines = vec![
                format!("Task: {}", values[0]),
                format!("Owner: {}", blank_dash(&values[1])),
                format!("Status: {}", blank_dash(&values[2])),
                format!("Due Date: {}", blank_dash(&values[3])),
                format!("Next Follow-Up: {}", blank_dash(&values[4])),
                format!("Last Reminder: {}", blank_dash(&values[5])),
                format!("Context: {}", blank_dash(&values[6])),
                format!("Updated: {}", record.updated_at),
            ];
            lines.insert(
                3,
                format!("Urgency: {}", self.spec.delegation_urgency(&values)),
            );
            if let Some(due_date) = parse_ymd(&values[3]) {
                let days_until = (due_date - Local::now().date_naive()).num_days();
                lines.insert(5, format!("Days Until Due: {days_until}"));
            }
            return lines;
        }
        if matches!(self.spec.kind, ToolKind::Decision) {
            let linked_notes = parse_linked_ids(&values[5]).len();
            let linked_tasks = parse_linked_ids(&values[6]).len();
            let mut lines = vec![
                format!("Decision: {}", values[0]),
                format!("Owner: {}", blank_dash(&values[1])),
                format!("Status: {}", blank_dash(&values[2])),
                format!("Date: {}", blank_dash(&values[3])),
                format!("Tags: {}", blank_dash(&values[4])),
                format!("Linked Notes: {}", blank_dash(&values[5])),
                format!("Linked Tasks: {}", blank_dash(&values[6])),
                format!(
                    "Resolved Links: {} notes | {} tasks",
                    linked_notes, linked_tasks
                ),
                format!("Rationale: {}", blank_dash(&values[7])),
                format!("Impact: {}", blank_dash(&values[8])),
                format!("Review Date: {}", blank_dash(&values[9])),
                format!(
                    "Review Status: {}",
                    self.spec.decision_review_status(&values)
                ),
                format!("Updated: {}", record.updated_at),
            ];
            if let Some(review_date) = parse_ymd(&values[9]) {
                let days_until = (review_date - Local::now().date_naive()).num_days();
                lines.insert(10, format!("Days Until Review: {days_until}"));
            }
            return lines;
        }
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

    fn bump_next_meeting(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(record) = self.records.get_mut(self.selected) else {
            return Ok(());
        };
        let mut values = self.spec.normalize_values(&record.values);
        let cadence = cadence_duration(&values[2]);
        let next_date = parse_ymd(&values[1])
            .map(|date| date + cadence)
            .unwrap_or_else(|| Local::now().date_naive() + cadence);
        values[1] = next_date.format("%Y-%m-%d").to_string();
        record.values = values;
        record.updated_at = timestamp();
        self.persist()?;
        self.log("Moved next 1:1 forward by cadence.");
        Ok(())
    }

    fn resolve_linked_records(&self) -> Result<Vec<LinkedRecord>, Box<dyn Error>> {
        if !matches!(self.spec.kind, ToolKind::Decision) {
            return Ok(Vec::new());
        }

        let Some(record) = self.selected_record() else {
            return Ok(Vec::new());
        };
        let values = self.spec.normalize_values(&record.values);
        let mut resolved = Vec::new();

        for note_id in parse_linked_ids(&values[5]) {
            match load_note_summary(note_id)? {
                Some((title, summary)) => resolved.push(LinkedRecord {
                    kind: LinkedRecordKind::Note,
                    id: note_id,
                    title,
                    summary,
                }),
                None => resolved.push(LinkedRecord {
                    kind: LinkedRecordKind::Note,
                    id: note_id,
                    title: format!("Missing note #{note_id}"),
                    summary: "Not found in notes database.".to_string(),
                }),
            }
        }

        for task_id in parse_linked_ids(&values[6]) {
            match load_task_summary(task_id)? {
                Some((title, summary)) => resolved.push(LinkedRecord {
                    kind: LinkedRecordKind::Task,
                    id: task_id,
                    title,
                    summary,
                }),
                None => resolved.push(LinkedRecord {
                    kind: LinkedRecordKind::Task,
                    id: task_id,
                    title: format!("Missing task #{task_id}"),
                    summary: "Not found in task database.".to_string(),
                }),
            }
        }

        Ok(resolved)
    }
}

impl ToolSpec {
    pub fn default_values(&self) -> Vec<String> {
        match self.kind {
            ToolKind::OneOnOne => vec![
                String::new(),
                Local::now().format("%Y-%m-%d").to_string(),
                "Weekly".to_string(),
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
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
        }
    }

    pub fn normalize_values(&self, values: &[String]) -> Vec<String> {
        if matches!(self.kind, ToolKind::OneOnOne) && values.len() == 5 {
            return vec![
                values.first().cloned().unwrap_or_default(),
                values.get(1).cloned().unwrap_or_default(),
                "Weekly".to_string(),
                String::new(),
                values.get(2).cloned().unwrap_or_default(),
                values.get(3).cloned().unwrap_or_default(),
                values.get(4).cloned().unwrap_or_default(),
            ];
        }
        if matches!(self.kind, ToolKind::Delegation) && values.len() == 5 {
            return vec![
                values.first().cloned().unwrap_or_default(),
                values.get(1).cloned().unwrap_or_default(),
                values
                    .get(2)
                    .cloned()
                    .unwrap_or_else(|| "Delegated".to_string()),
                values.get(3).cloned().unwrap_or_default(),
                String::new(),
                String::new(),
                values.get(4).cloned().unwrap_or_default(),
            ];
        }
        if matches!(self.kind, ToolKind::Decision) && values.len() == 6 {
            return vec![
                values.first().cloned().unwrap_or_default(),
                values.get(1).cloned().unwrap_or_default(),
                values
                    .get(2)
                    .cloned()
                    .unwrap_or_else(|| "Proposed".to_string()),
                values
                    .get(3)
                    .cloned()
                    .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string()),
                String::new(),
                String::new(),
                String::new(),
                values.get(4).cloned().unwrap_or_default(),
                values.get(5).cloned().unwrap_or_default(),
                String::new(),
            ];
        }
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
                    "next {} | {} cadence | follow-ups {}",
                    blank_dash(&values[1]),
                    blank_dash(&values[2]),
                    blank_dash(&values[5])
                )
            }
            ToolKind::Delegation => format!(
                "{} | {} | {}",
                blank_dash(&values[2]),
                with_prefix("due ", &values[3]).replace(" -", "-"),
                with_prefix("follow-up ", &values[4]).replace(" -", "-")
            ),
            ToolKind::Decision => {
                format!(
                    "{} | {} | tags {}",
                    blank_dash(&values[2]),
                    with_prefix("review ", &values[9]).replace(" -", "-"),
                    blank_dash(&values[4])
                )
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

    pub fn matches_filter(&self, record: &ToolRecord, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }

        let values = self.normalize_values(&record.values);
        let lower_all = values.join(" ").to_lowercase();

        for token in trimmed.split_whitespace() {
            let token = token.to_lowercase();
            if let Some(rest) = token.strip_prefix("person:") {
                if !contains_value(values.first(), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("owner:") {
                if !contains_value(values.get(1), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("status:") {
                if !contains_str(self.status_value(&values), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("date:") {
                if !contains_str(self.date_value(&values), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("tag:") {
                if !contains_str(values.get(4).map(|value| value.as_str()), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("note:") {
                if !contains_str(values.get(5).map(|value| value.as_str()), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("task:") {
                if !contains_str(values.get(6).map(|value| value.as_str()), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("cadence:") {
                if !contains_str(values.get(2).map(|value| value.as_str()), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("due:") {
                if !contains_str(self.due_value(&values), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("followup:") {
                if !contains_str(self.follow_up_value(&values), rest) {
                    return false;
                }
            } else if token == "review" || token == "reviews" {
                if !self.review_due(&values) {
                    return false;
                }
            } else if token == "overdue" {
                if !self.is_overdue(&values) {
                    return false;
                }
            } else if token == "reminder" || token == "reminders" {
                if !self.needs_follow_up(&values) {
                    return false;
                }
            } else if token == "followup" || token == "followups" {
                let follow_up_index = if matches!(self.kind, ToolKind::OneOnOne) {
                    5
                } else if matches!(self.kind, ToolKind::Delegation) {
                    4
                } else {
                    3
                };
                if values
                    .get(follow_up_index)
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
                {
                    return false;
                }
            } else if !lower_all.contains(&token) {
                return false;
            }
        }

        true
    }

    fn status_value<'a>(&self, values: &'a [String]) -> Option<&'a str> {
        self.status_field_index()
            .and_then(|index| values.get(index))
            .map(|value| value.as_str())
    }

    fn date_value<'a>(&self, values: &'a [String]) -> Option<&'a str> {
        match self.kind {
            ToolKind::OneOnOne => values.get(1).map(|value| value.as_str()),
            ToolKind::Decision => values.get(3).map(|value| value.as_str()),
            ToolKind::Delegation => None,
        }
    }

    fn due_value<'a>(&self, values: &'a [String]) -> Option<&'a str> {
        match self.kind {
            ToolKind::Delegation => values.get(3).map(|value| value.as_str()),
            ToolKind::OneOnOne | ToolKind::Decision => None,
        }
    }

    fn follow_up_value<'a>(&self, values: &'a [String]) -> Option<&'a str> {
        match self.kind {
            ToolKind::Delegation => values.get(4).map(|value| value.as_str()),
            ToolKind::OneOnOne | ToolKind::Decision => None,
        }
    }

    pub fn is_overdue(&self, values: &[String]) -> bool {
        match self.kind {
            ToolKind::Delegation => {
                let Some(due_date) = values.get(3).and_then(|value| parse_ymd(value)) else {
                    return false;
                };
                let done = values.get(2).map(|value| value == "Done").unwrap_or(false);
                !done && due_date < Local::now().date_naive()
            }
            ToolKind::OneOnOne | ToolKind::Decision => false,
        }
    }

    pub fn needs_follow_up(&self, values: &[String]) -> bool {
        match self.kind {
            ToolKind::Delegation => {
                if values.get(2).map(|value| value == "Done").unwrap_or(false) {
                    return false;
                }

                if self.is_overdue(values) {
                    return true;
                }

                values
                    .get(4)
                    .and_then(|value| parse_ymd(value))
                    .map(|date| date <= Local::now().date_naive())
                    .unwrap_or(false)
            }
            ToolKind::OneOnOne | ToolKind::Decision => false,
        }
    }

    pub fn delegation_urgency(&self, values: &[String]) -> &'static str {
        if !matches!(self.kind, ToolKind::Delegation) {
            return "Normal";
        }
        if values.get(2).map(|value| value == "Done").unwrap_or(false) {
            return "Closed";
        }
        if self.is_overdue(values) {
            return "Overdue";
        }
        if self.needs_follow_up(values) {
            return "Follow-up due";
        }
        "On track"
    }

    pub fn review_due(&self, values: &[String]) -> bool {
        match self.kind {
            ToolKind::Decision => values
                .get(9)
                .and_then(|value| parse_ymd(value))
                .map(|date| date <= Local::now().date_naive())
                .unwrap_or(false),
            ToolKind::OneOnOne | ToolKind::Delegation => false,
        }
    }

    pub fn decision_review_status(&self, values: &[String]) -> &'static str {
        if !matches!(self.kind, ToolKind::Decision) {
            return "N/A";
        }
        if self.review_due(values) {
            return "Review due";
        }
        if values
            .get(9)
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
        {
            return "No review scheduled";
        }
        "Scheduled"
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
                .filter(|record| !value_at(record, 1).trim().is_empty())
                .count(),
            records
                .iter()
                .filter(|record| !value_at(record, 3).trim().is_empty())
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
                .filter(|record| {
                    let values = spec.normalize_values(&record.values);
                    spec.needs_follow_up(&values)
                })
                .count(),
            "open",
            "follow-up due",
        ),
        ToolKind::Decision => (
            records
                .iter()
                .filter(|record| value_at(record, 2) == "Decided")
                .count(),
            records
                .iter()
                .filter(|record| {
                    let values = spec.normalize_values(&record.values);
                    spec.review_due(&values)
                })
                .count(),
            "decided",
            "review due",
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

fn load_task_summary(task_id: i32) -> Result<Option<(String, String)>, Box<dyn Error>> {
    let db_path = crate::db::resolve_db_path(
        "TASK_MANAGER_DB_DIR",
        ".task_manager",
        "TASK_MANAGER_DB_FILENAME",
        "task_manager.db",
    );
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = crate::db::establish_connection_pool(&db_url)?;
    let ops = crate::db::task_manager::operations::DbOperations::new(pool);

    Ok(ops.find_task(task_id)?.map(|task| {
        let status = if task.completed { "done" } else { "open" };
        (
            task.name,
            format!("#{task_id} | {status} | {}", blank_dash(&task.description)),
        )
    }))
}

fn load_note_summary(note_id: i32) -> Result<Option<(String, String)>, Box<dyn Error>> {
    let db_path =
        crate::db::resolve_db_path("NOTES_DB_DIR", ".notes", "NOTES_DB_FILENAME", "notes.db");
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = crate::db::establish_connection_pool(&db_url)?;
    let ops = crate::db::notes::operations::DbOperations::new(pool);

    Ok(ops.find_note(note_id)?.map(|note| {
        (
            note.title,
            format!("#{note_id} | {}", summarize_inline(&note.content, 48)),
        )
    }))
}

fn parse_ymd(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()
}

fn parse_linked_ids(value: &str) -> Vec<i32> {
    let mut ids = Vec::new();

    for token in value
        .split(|c: char| c == ',' || c == '|' || c.is_whitespace())
        .filter(|part| !part.trim().is_empty())
    {
        let token = token
            .trim()
            .trim_start_matches("note:")
            .trim_start_matches("task:")
            .trim_start_matches('#');
        if let Ok(id) = token.parse::<i32>() {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }

    ids
}

fn cadence_duration(value: &str) -> Duration {
    let normalized = value.trim().to_lowercase();
    if normalized.contains("biweekly") || normalized.contains("bi-weekly") {
        Duration::days(14)
    } else if normalized.contains("monthly") {
        Duration::days(28)
    } else {
        Duration::days(7)
    }
}

fn merge_rollover_items(existing: &str, agenda: &str) -> String {
    let agenda = agenda.trim();
    let existing = existing.trim();

    match (existing.is_empty(), agenda.is_empty()) {
        (_, true) => existing.to_string(),
        (true, false) => agenda.to_string(),
        (false, false) => format!("{existing} | {agenda}"),
    }
}

fn contains_value(value: Option<&String>, needle: &str) -> bool {
    value
        .map(|value| value.to_lowercase().contains(needle))
        .unwrap_or(false)
}

fn contains_str(value: Option<&str>, needle: &str) -> bool {
    value
        .map(|value| value.to_lowercase().contains(needle))
        .unwrap_or(false)
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

fn summarize_inline(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = compact.chars();
    let summary = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{summary}...")
    } else if summary.is_empty() {
        "-".to_string()
    } else {
        summary
    }
}
