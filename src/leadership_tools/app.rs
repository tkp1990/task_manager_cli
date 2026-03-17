mod helpers;
mod storage;
mod types;

use chrono::{Duration, Local};
use helpers::*;
use std::error::Error;
use std::fs;
pub use storage::load_dashboard_snapshot;
use storage::*;
pub use types::{
    App, DashboardSnapshot, InputMode, LinkedRecord, LinkedRecordKind, ToolKind, ToolRecord,
    ToolSpec,
};

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

    pub fn extract_sync_actions(&mut self) -> Result<(), Box<dyn Error>> {
        if !matches!(self.spec.kind, ToolKind::OneOnOne) {
            return Ok(());
        }

        let Some(record) = self.records.get_mut(self.selected) else {
            return Ok(());
        };

        let mut values = self.spec.normalize_values(&record.values);
        let actions = parse_action_items(&values[10]);
        if actions.is_empty() {
            self.feedback = Some("No action items to extract.".to_string());
            return Ok(());
        }

        append_delegations_from_sync(&values[0], &values[3], &values[8], &values[2], &actions)?;
        values[11] = merge_rollover_items(&values[11], &values[10]);
        values[10].clear();
        record.values = values;
        record.updated_at = timestamp();
        self.persist()?;
        self.feedback = Some(format!(
            "Extracted {} action item(s) into Delegation Tracker.",
            actions.len()
        ));
        self.log(&format!(
            "Extracted {} action item(s) into Delegation Tracker.",
            actions.len()
        ));
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
        let meeting_date = parse_ymd(&values[5]).unwrap_or_else(|| Local::now().date_naive());
        let cadence = cadence_duration(&values[6]);
        let next_date = meeting_date + cadence;

        values[7] = meeting_date.format("%Y-%m-%d").to_string();
        values[5] = next_date.format("%Y-%m-%d").to_string();
        values[11] =
            merge_rollover_items(&values[11], &merge_rollover_items(&values[9], &values[10]));
        values[9].clear();
        values[10].clear();

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
                format!("Relationship: {}", blank_dash(&values[1])),
                format!("Meeting Type: {}", blank_dash(&values[2])),
                format!("Team / Org: {}", blank_dash(&values[3])),
                format!("Manager / Chain: {}", blank_dash(&values[4])),
                format!("Next 1:1: {}", blank_dash(&values[5])),
                format!("Cadence: {}", blank_dash(&values[6])),
                format!("Last 1:1: {}", blank_dash(&values[7])),
                format!("Purpose: {}", blank_dash(&values[8])),
                format!("Agenda: {}", blank_dash(&values[9])),
                format!("Action Items: {}", blank_dash(&values[10])),
                format!("Follow-Ups: {}", blank_dash(&values[11])),
                format!(
                    "Agenda Ready: {}",
                    if values[9].trim().is_empty() && values[10].trim().is_empty() {
                        "No"
                    } else {
                        "Yes"
                    }
                ),
                format!("Private Notes: {}", blank_dash(&values[12])),
                format!("Updated: {}", record.updated_at),
            ];
            if let Some(next_date) = parse_ymd(&values[5]) {
                let days_until = (next_date - Local::now().date_naive()).num_days();
                lines.insert(5, format!("Days Until Next: {days_until}"));
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
        let cadence = cadence_duration(&values[6]);
        let next_date = parse_ymd(&values[5])
            .map(|date| date + cadence)
            .unwrap_or_else(|| Local::now().date_naive() + cadence);
        values[5] = next_date.format("%Y-%m-%d").to_string();
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
                "Direct Report".to_string(),
                "1:1".to_string(),
                String::new(),
                String::new(),
                Local::now().format("%Y-%m-%d").to_string(),
                "Weekly".to_string(),
                String::new(),
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
                "Direct Report".to_string(),
                "1:1".to_string(),
                String::new(),
                String::new(),
                values.get(1).cloned().unwrap_or_default(),
                "Weekly".to_string(),
                String::new(),
                String::new(),
                values.get(2).cloned().unwrap_or_default(),
                String::new(),
                values.get(3).cloned().unwrap_or_default(),
                values.get(4).cloned().unwrap_or_default(),
            ];
        }
        if matches!(self.kind, ToolKind::OneOnOne) && values.len() == 7 {
            return vec![
                values.first().cloned().unwrap_or_default(),
                "Direct Report".to_string(),
                "1:1".to_string(),
                String::new(),
                String::new(),
                values.get(1).cloned().unwrap_or_default(),
                values
                    .get(2)
                    .cloned()
                    .unwrap_or_else(|| "Weekly".to_string()),
                values.get(3).cloned().unwrap_or_default(),
                String::new(),
                values.get(4).cloned().unwrap_or_default(),
                String::new(),
                values.get(5).cloned().unwrap_or_default(),
                values.get(6).cloned().unwrap_or_default(),
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
                    "{} | {} | next {}",
                    blank_dash(&values[1]),
                    blank_dash(&values[3]),
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
            } else if let Some(rest) = token.strip_prefix("relationship:") {
                if !contains_value(values.get(1), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("type:") {
                if !contains_value(values.get(2), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("team:") {
                if !contains_value(values.get(3), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("manager:") {
                if !contains_value(values.get(4), rest) {
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
                if !contains_str(values.get(6).map(|value| value.as_str()), rest) {
                    return false;
                }
            } else if let Some(rest) = token.strip_prefix("purpose:") {
                if !contains_str(values.get(8).map(|value| value.as_str()), rest) {
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
                    11
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
            } else if token == "actions" || token == "actionitems" {
                if values
                    .get(10)
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
            ToolKind::OneOnOne => values.get(5).map(|value| value.as_str()),
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
