use crate::common::command_palette;
use crate::common::widgets;
use crate::task_manager::app::{App, InputMode};
use crate::ui_style::{self, Accent, PopupSize};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};

use super::events::visible_task_palette_commands;

fn highlighted_spans(text: &str, query: &str, base: Style, highlight: Style) -> Spans<'static> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Spans::from(Span::styled(text.to_string(), base));
    }

    let lower_text = text.to_lowercase();
    let lower_query = trimmed.to_lowercase();
    let mut spans = Vec::new();
    let mut cursor = 0;

    while let Some(relative) = lower_text[cursor..].find(&lower_query) {
        let start = cursor + relative;
        let end = start + lower_query.len();
        if start > cursor {
            spans.push(Span::styled(text[cursor..start].to_string(), base));
        }
        spans.push(Span::styled(text[start..end].to_string(), highlight));
        cursor = end;
    }

    if cursor < text.len() {
        spans.push(Span::styled(text[cursor..].to_string(), base));
    }

    Spans::from(spans)
}

fn task_status_spans(task: &crate::db::task_manager::models::Task) -> Spans<'static> {
    let mut spans = vec![Span::styled(
        if task.completed { "DONE " } else { "OPEN " },
        if task.completed {
            ui_style::success_style()
        } else {
            ui_style::warning_style()
        },
    )];

    if task.favourite {
        spans.push(Span::styled(
            "STAR ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::styled(
        format!("Updated {}", task.updated_at),
        ui_style::subtle_style(),
    ));

    Spans::from(spans)
}

pub fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Length(15),
            ]
            .as_ref(),
        )
        .split(size);

    let titles: Vec<Spans> = if app.topics.is_empty() {
        vec![Spans::from(Span::styled(
            "No topics. Press 'N' to add a topic.",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))]
    } else {
        app.topics
            .iter()
            .map(|t| Spans::from(Span::raw(&t.name)))
            .collect()
    };
    let topic_title = format!("Topics [{}]", app.topics.len());
    let tabs = Tabs::new(titles)
        .select(if app.topics.is_empty() {
            0
        } else {
            app.selected_topic
        })
        .block(ui_style::surface_block(&topic_title, Accent::Tasks))
        .highlight_style(ui_style::title_style(Accent::Tasks))
        .divider(Span::raw("|"));
    f.render_widget(tabs, chunks[0]);

    let filtered_indices = app.filtered_task_indices();
    let items: Vec<ListItem> = if app.tasks.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No tasks in this topic. Press 'a' to add a task.",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))])]
    } else if filtered_indices.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            format!("No tasks match \"{}\".", app.task_filter),
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ))])]
    } else {
        filtered_indices
            .iter()
            .map(|task_index| {
                let task = &app.tasks[*task_index];
                let title_style = if task.completed {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::CROSSED_OUT)
                } else {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                };
                let summary = if task.description.trim().is_empty() {
                    "No description".to_string()
                } else if task.description.len() > 72 {
                    format!("{}...", &task.description[..72])
                } else {
                    task.description.clone()
                };
                let lines = if app.expanded.contains(&task.id) {
                    vec![
                        highlighted_spans(
                            &task.name,
                            &app.task_filter,
                            title_style,
                            ui_style::focused_inline_style(),
                        ),
                        highlighted_spans(
                            &summary,
                            &app.task_filter,
                            ui_style::info_style(),
                            ui_style::focused_inline_style(),
                        ),
                        task_status_spans(task),
                        Spans::from(Span::styled(
                            format!(
                                "ID {} | Created {} | Topic {}",
                                task.id, task.created_at, task.topic_id
                            ),
                            ui_style::muted_style(),
                        )),
                    ]
                } else {
                    vec![
                        highlighted_spans(
                            &task.name,
                            &app.task_filter,
                            title_style,
                            ui_style::focused_inline_style(),
                        ),
                        highlighted_spans(
                            &summary,
                            &app.task_filter,
                            ui_style::info_style(),
                            ui_style::focused_inline_style(),
                        ),
                        task_status_spans(task),
                    ]
                };
                ListItem::new(lines)
            })
            .collect()
    };

    let tasks_title = if app.has_task_filter() {
        format!(
            "Tasks [shown {} / total {}] | Filter: {}",
            filtered_indices.len(),
            app.tasks.len(),
            app.task_filter
        )
    } else {
        format!(
            "Tasks [shown {} / total {}]",
            filtered_indices.len(),
            app.tasks.len()
        )
    };
    let tasks_list = List::new(items)
        .block(ui_style::surface_block(&tasks_title, Accent::Tasks))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut list_state = ListState::default();
    if !filtered_indices.is_empty() {
        list_state.select(
            filtered_indices
                .iter()
                .position(|index| *index == app.selected),
        );
    }
    f.render_stateful_widget(tasks_list, chunks[1], &mut list_state);

    let command_lines =
        match app.input_mode {
            InputMode::Normal => vec![
                ui_style::command_bar_spans(&[
                    ("a", "add task"),
                    ("A", "add topic"),
                    ("e", "edit"),
                    ("d", "delete"),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "expand"),
                    ("Space", "done"),
                    ("/", "filter"),
                    ("p", "presets"),
                    (":", "palette"),
                ]),
                ui_style::command_bar_spans(&[
                    ("S", "special"),
                    ("f", "favorite"),
                    ("H", "help"),
                    ("q", "quit"),
                ]),
            ],
            InputMode::Filtering => vec![
                Spans::from(vec![
                    Span::raw("Query "),
                    Span::styled(
                        app.task_filter.clone(),
                        ui_style::title_style(Accent::Tasks),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "keep filter"),
                    ("Esc", "clear"),
                    ("status:", "done/open"),
                    ("topic:", "topic"),
                    ("fav:", "favorite"),
                ]),
            ],
            InputMode::CommandPalette => vec![
                Spans::from(vec![
                    Span::raw("Palette "),
                    Span::styled(
                        app.command_palette_query.clone(),
                        ui_style::title_style(Accent::Tasks),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "run command"),
                    ("j/k", "move"),
                    ("Backspace", "edit query"),
                    ("Esc", "close"),
                ]),
            ],
            InputMode::AddingTopic => vec![
                Spans::from(vec![
                    Span::raw("Topic "),
                    Span::styled(app.input.clone(), ui_style::title_style(Accent::Tasks)),
                ]),
                ui_style::command_bar_spans(&[("Enter", "create"), ("Esc", "cancel")]),
            ],
            InputMode::ViewingSpecialTopics => vec![
                ui_style::command_bar_spans(&[
                    ("Tab", "switch tab"),
                    ("/", "filter"),
                    ("p", "presets"),
                    ("d", "delete"),
                    ("f", "favorite"),
                ]),
                ui_style::command_bar_spans(&[(":", "palette"), ("Esc", "close"), ("H", "help")]),
            ],
            InputMode::FilteringSpecial => vec![
                Spans::from(vec![
                    Span::raw("Special "),
                    Span::styled(
                        app.special_task_filter.clone(),
                        ui_style::title_style(Accent::Tasks),
                    ),
                ]),
                ui_style::command_bar_spans(&[
                    ("Enter", "keep filter"),
                    ("Esc", "clear"),
                    ("status:", "done/open"),
                    ("fav:", "favorite"),
                ]),
            ],
            InputMode::PresetFilters | InputMode::PresetSpecialFilters => {
                vec![ui_style::command_bar_spans(&[
                    ("Enter", "apply"),
                    ("S", "save current"),
                    ("x", "delete saved"),
                    ("Esc", "close"),
                ])]
            }
            InputMode::SavingPreset | InputMode::SavingSpecialPreset => vec![
                ui_style::command_bar_spans(&[("Enter", "save preset"), ("Esc", "cancel")]),
            ],
            InputMode::DeleteTask | InputMode::DeleteSpecialTask => vec![
                ui_style::command_bar_spans(&[("y", "confirm delete"), ("n", "cancel")]),
            ],
            InputMode::Help => vec![ui_style::command_bar_spans(&[("Esc", "close help")])],
            InputMode::AddingTaskName
            | InputMode::AddingTaskDescription
            | InputMode::EditingTaskName
            | InputMode::EditingTaskDescription => vec![ui_style::command_bar_spans(&[
                ("Tab", "switch field"),
                ("Enter", "save"),
                ("Esc", "cancel"),
            ])],
        };
    let help_message = Paragraph::new(command_lines)
        .style(ui_style::info_style())
        .block(ui_style::command_bar_block("Commands"));
    f.render_widget(help_message, chunks[2]);

    let mode_text = match app.input_mode {
        InputMode::Normal => "Normal Mode",
        InputMode::CommandPalette => "Command Palette",
        InputMode::Filtering => "Filtering Tasks",
        InputMode::AddingTaskName => "Adding Task - Name Input",
        InputMode::AddingTaskDescription => "Adding Task - Description Input",
        InputMode::EditingTaskName => "Editing Task - Name Input",
        InputMode::EditingTaskDescription => "Editing Task - Description Input",
        InputMode::PresetFilters => "Task Presets",
        InputMode::PresetSpecialFilters => "Special Task Presets",
        InputMode::SavingPreset => "Saving Task Preset",
        InputMode::SavingSpecialPreset => "Saving Special Preset",
        InputMode::DeleteTask => "Delete Task",
        InputMode::DeleteSpecialTask => "Delete Task",
        InputMode::AddingTopic => "Adding Topic",
        InputMode::Help => "Viewing Help",
        InputMode::ViewingSpecialTopics => "Viewing Special Topics",
        InputMode::FilteringSpecial => "Filtering Special Tasks",
    };
    let mode = Paragraph::new(mode_text)
        .style(ui_style::body_style())
        .block(ui_style::shell_block("Mode"));
    f.render_widget(mode, chunks[3]);

    let log_area_height = chunks[4].height as usize;
    let total_logs = app.logs.len();
    let start = if total_logs > log_area_height + app.log_offset {
        total_logs - log_area_height - app.log_offset
    } else {
        0
    };
    let visible_logs: Vec<ListItem> = app.logs[start..]
        .iter()
        .map(|line| ListItem::new(Span::raw(line)))
        .collect();
    let logs_list = List::new(visible_logs).block(ui_style::shell_block("Logs"));
    f.render_widget(logs_list, chunks[4]);

    if app.input_mode == InputMode::Help {
        let help_text = get_help_text();
        let help_paragraph = Paragraph::new(help_text)
            .block(ui_style::popup_block("Help", Accent::Tasks))
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .wrap(Wrap { trim: true });
        let area = ui_style::popup_rect(PopupSize::Tall, size);
        f.render_widget(Clear, area);
        f.render_widget(help_paragraph, area);
    }

    if matches!(
        app.input_mode,
        InputMode::AddingTaskName
            | InputMode::AddingTaskDescription
            | InputMode::EditingTaskName
            | InputMode::EditingTaskDescription
    ) {
        draw_add_task_popup(f, app);
    }

    if app.input_mode == InputMode::DeleteTask {
        draw_delete_popup(f, app);
    }

    if matches!(
        app.input_mode,
        InputMode::ViewingSpecialTopics
            | InputMode::FilteringSpecial
            | InputMode::PresetSpecialFilters
            | InputMode::DeleteSpecialTask
    ) {
        draw_special_topics_popup(f, app);
    }

    if app.input_mode == InputMode::PresetFilters {
        draw_task_presets_popup(f, app, false);
    }

    if matches!(
        app.input_mode,
        InputMode::SavingPreset | InputMode::SavingSpecialPreset
    ) {
        draw_save_task_preset_popup(f, app);
    }

    if app.input_mode == InputMode::CommandPalette {
        draw_command_palette_popup(f, app, size);
    }
}

fn draw_command_palette_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
    let commands = visible_task_palette_commands(app);
    command_palette::draw_popup(
        f,
        size,
        app.command_palette_query.as_str(),
        app.command_palette_selected,
        &commands,
        Accent::Tasks,
    );
}

fn draw_add_task_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let popup_area = ui_style::popup_rect(PopupSize::Standard, size);
    f.render_widget(Clear, popup_area);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(popup_area);

    let popup_title = Paragraph::new(
        if matches!(
            app.input_mode,
            InputMode::EditingTaskName | InputMode::EditingTaskDescription
        ) {
            "Edit Task"
        } else {
            "Create New Task"
        },
    )
    .style(ui_style::title_style(Accent::Tasks))
    .block(ui_style::popup_block("Task Editor", Accent::Tasks));
    f.render_widget(popup_title, popup_layout[0]);

    let name_input_style = if matches!(
        app.input_mode,
        InputMode::AddingTaskName | InputMode::EditingTaskName
    ) {
        ui_style::title_style(Accent::Tasks)
    } else {
        ui_style::muted_style()
    };

    let name_input = Paragraph::new(app.task_name_input.as_ref())
        .style(name_input_style)
        .block(ui_style::popup_block("Task Name", Accent::Tasks));
    f.render_widget(name_input, popup_layout[1]);

    let desc_input_style = if matches!(
        app.input_mode,
        InputMode::AddingTaskDescription | InputMode::EditingTaskDescription
    ) {
        ui_style::title_style(Accent::Tasks)
    } else {
        ui_style::muted_style()
    };

    let desc_input = Paragraph::new(app.task_description_input.as_ref())
        .style(desc_input_style)
        .block(ui_style::popup_block("Task Description", Accent::Tasks))
        .wrap(Wrap { trim: true });
    f.render_widget(desc_input, popup_layout[3]);

    let instructions = match app.input_mode {
        InputMode::AddingTaskName => "Enter task name and press Enter to continue. (Esc to cancel)",
        InputMode::AddingTaskDescription => {
            "Enter task description and press Enter to save. (Tab to edit name, Esc to cancel)"
        }
        InputMode::EditingTaskName => "Edit task name and press Enter to continue. (Esc to cancel)",
        InputMode::EditingTaskDescription => {
            "Edit task description and press Enter to save. (Tab to edit name, Esc to cancel)"
        }
        _ => "",
    };

    let feedback_text = app
        .task_form_message
        .clone()
        .unwrap_or_else(|| "Enter a name, then a description, then save.".to_string());
    let feedback_style = if app.task_form_message.is_some() {
        ui_style::danger_style()
    } else {
        ui_style::subtle_style()
    };
    let feedback = Paragraph::new(feedback_text)
        .style(feedback_style)
        .block(ui_style::popup_block("Feedback", Accent::Tasks));
    f.render_widget(feedback, popup_layout[4]);

    let instructions_text = Paragraph::new(instructions)
        .style(ui_style::body_style())
        .block(ui_style::popup_block("Instructions", Accent::Tasks));
    f.render_widget(instructions_text, popup_layout[5]);

    if matches!(
        app.input_mode,
        InputMode::AddingTaskName | InputMode::EditingTaskName
    ) {
        f.set_cursor(
            popup_layout[1].x + app.task_name_input.len() as u16 + 1,
            popup_layout[1].y + 1,
        );
    } else if matches!(
        app.input_mode,
        InputMode::AddingTaskDescription | InputMode::EditingTaskDescription
    ) {
        f.set_cursor(
            popup_layout[3].x + app.task_description_input.len() as u16 + 1,
            popup_layout[3].y + 1,
        );
    }
}

fn draw_delete_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let task_name = if let Some(task) = app.tasks.get(app.selected) {
        &task.name
    } else {
        "Unknown Task"
    };
    widgets::draw_confirmation_popup(
        f,
        f.size(),
        Accent::Tasks,
        "Delete Task",
        "Delete Confirmation",
        &format!("Are you sure you want to delete \"{}\"?", task_name),
        "Press [Y] to confirm deletion or [N] to cancel",
    );
}

fn build_help_line(
    title: &'static str,
    key: &'static str,
    description: &'static str,
) -> Spans<'static> {
    Spans::from(vec![
        Span::styled(title, ui_style::info_style().add_modifier(Modifier::BOLD)),
        Span::raw(" Press "),
        Span::styled(key, ui_style::title_style(Accent::Tasks)),
        Span::raw(" "),
        Span::styled(description, ui_style::body_style()),
    ])
}

pub fn get_help_text() -> Vec<Spans<'static>> {
    vec![
        Spans::from(Span::styled(
            "Help - Available Operations",
            ui_style::title_style(Accent::Tasks),
        )),
        Spans::from(""),
        build_help_line(
            "Add Task:",
            "'a'",
            "opens a popup to create a new task with name and description.",
        ),
        build_help_line(
            "Filter Tasks:",
            "'/'",
            "filter live. Supports status:done, topic:work, fav:true, quoted phrases, and -negation.",
        ),
        build_help_line("Task Presets:", "'p'", "open saved preset filters for quick reuse."),
        build_help_line(
            "Edit Task:",
            "'e'",
            "opens the same two-field form used for task creation.",
        ),
        build_help_line("Toggle Complete:", "'t'", "to mark a task complete/incomplete."),
        build_help_line("Toggle Favourite:", "'f'", "to mark/unmark as favourite."),
        build_help_line("Delete Task:", "'d'", "to delete the selected task."),
        build_help_line("Expand/Collapse Task:", "Enter", "to toggle details."),
        build_help_line("Navigate Tasks:", "Up/Down or j/k", "to move between tasks."),
        build_help_line("Switch Topics:", "Left/Right or h/l", "to change topics."),
        build_help_line("Add Topic:", "'N'", "to add a new topic."),
        build_help_line(
            "Delete Topic:",
            "'X'",
            "to delete the current topic (Favourites is protected).",
        ),
        build_help_line("Scroll Logs:", "PageUp/PageDown", "to scroll logs."),
        build_help_line(
            "Open Favourites/Completed:",
            "Shift+W",
            "open a floating window with Favourites and Completed tabs.",
        ),
        build_help_line(
            "Switch Special Tabs:",
            "Left/Right or h/l",
            "switch between Favourites and Completed in the popup.",
        ),
        build_help_line(
            "Navigate Special Tasks:",
            "Up/Down or j/k",
            "navigate tasks in the special popup.",
        ),
        build_help_line(
            "Special Popup Actions:",
            "t/f/d/Enter",
            "toggle complete/favourite, delete, or expand in popup.",
        ),
        build_help_line("Close Popup:", "Esc", "close the Favourites/Completed window."),
        build_help_line("Toggle Help:", "'H'", "to show/hide help."),
        build_help_line("Quit:", "'q'", "to exit the application."),
    ]
}

fn draw_special_topics_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let popup_area = ui_style::popup_rect(PopupSize::Full, size);
    f.render_widget(Clear, popup_area);
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(popup_area);

    let tab_titles = vec![Spans::from("Favourites"), Spans::from("Completed")];
    let tabs = Tabs::new(tab_titles)
        .select(app.special_tab_selected)
        .block(ui_style::popup_block("Special Tasks", Accent::Tasks))
        .highlight_style(ui_style::title_style(Accent::Tasks))
        .divider(Span::raw("|"));
    f.render_widget(tabs, popup_layout[0]);

    let tasks = app.get_current_special_tasks();
    let filtered_indices = app.filtered_special_task_indices();
    let items: Vec<ListItem> = if tasks.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No tasks found.",
            ui_style::muted_style().add_modifier(Modifier::ITALIC),
        ))])]
    } else if filtered_indices.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            format!("No tasks match \"{}\".", app.special_task_filter),
            ui_style::muted_style().add_modifier(Modifier::ITALIC),
        ))])]
    } else {
        filtered_indices
            .iter()
            .map(|task_index| {
                let task = &tasks[*task_index];
                let description_style = if task.completed {
                    ui_style::success_style()
                } else {
                    ui_style::info_style()
                };
                let lines = if app.expanded.contains(&task.id) {
                    vec![
                        highlighted_spans(
                            &task.name,
                            &app.special_task_filter,
                            description_style,
                            ui_style::focused_inline_style(),
                        ),
                        highlighted_spans(
                            &format!("Description: {}", task.description),
                            &app.special_task_filter,
                            description_style,
                            ui_style::focused_inline_style(),
                        ),
                        Spans::from(Span::styled(
                            format!(
                                "ID: {} | Completed: {} | Favourite: {} | Created: {} | Updated: {}",
                                task.id,
                                if task.completed { "Yes" } else { "No" },
                                if task.favourite { "Yes" } else { "No" },
                                task.created_at,
                                task.updated_at
                            ),
                            ui_style::muted_style(),
                        )),
                    ]
                } else {
                    vec![highlighted_spans(
                        &format!("{}: {}", task.name, task.description),
                        &app.special_task_filter,
                        description_style,
                        ui_style::focused_inline_style(),
                    )]
                };
                ListItem::new(lines)
            })
            .collect()
    };

    let tasks_title = if app.has_special_task_filter() {
        format!(
            "Tasks [shown {} / total {}] | Filter: {}",
            filtered_indices.len(),
            tasks.len(),
            app.special_task_filter
        )
    } else {
        format!(
            "Tasks [shown {} / total {}]",
            filtered_indices.len(),
            tasks.len()
        )
    };
    let tasks_list = List::new(items)
        .block(ui_style::popup_block(&tasks_title, Accent::Tasks))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut list_state = ListState::default();
    if !filtered_indices.is_empty() {
        list_state.select(
            filtered_indices
                .iter()
                .position(|index| *index == app.special_task_selected),
        );
    }
    f.render_stateful_widget(tasks_list, popup_layout[1], &mut list_state);

    let instructions = if app.input_mode == InputMode::DeleteSpecialTask {
        "Press [Y] to confirm deletion or [N] to cancel"
    } else if app.input_mode == InputMode::PresetSpecialFilters {
        "Preset filters. Enter to apply, Esc to cancel"
    } else if app.input_mode == InputMode::FilteringSpecial {
        "Filter special tasks. Enter to keep filter, Esc to clear"
    } else {
        "Up/Down: Navigate | /: Filter | p: Presets | Enter: Expand | t: Toggle | f: Favourite | d: Delete | Esc: Close"
    };
    let instructions_text = Paragraph::new(instructions)
        .style(ui_style::info_style())
        .block(ui_style::popup_block("Instructions", Accent::Tasks));
    f.render_widget(instructions_text, popup_layout[2]);

    if app.input_mode == InputMode::DeleteSpecialTask {
        let tasks = app.get_current_special_tasks();
        if let Some(task) = tasks.get(app.special_task_selected) {
            let delete_area = ui_style::popup_rect(PopupSize::Compact, size);
            f.render_widget(Clear, delete_area);
            let delete_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(delete_area);

            let delete_title = Paragraph::new("Delete Confirmation")
                .style(ui_style::title_style(Accent::Tasks))
                .alignment(tui::layout::Alignment::Center)
                .block(ui_style::popup_block("Delete Task", Accent::Tasks));
            f.render_widget(delete_title, delete_layout[0]);

            let delete_msg = Paragraph::new(format!(
                "Are you sure you want to delete \"{}\"?",
                task.name
            ))
            .style(ui_style::danger_style())
            .alignment(tui::layout::Alignment::Center)
            .block(ui_style::popup_block("Confirmation", Accent::Tasks));
            f.render_widget(delete_msg, delete_layout[1]);

            let delete_instructions = Paragraph::new("Press [Y] to confirm or [N] to cancel")
                .style(ui_style::info_style())
                .alignment(tui::layout::Alignment::Center)
                .block(ui_style::popup_block("Controls", Accent::Tasks));
            f.render_widget(delete_instructions, delete_layout[2]);
        }
    }

    if app.input_mode == InputMode::PresetSpecialFilters {
        draw_task_presets_popup(f, app, true);
    }
}

fn draw_task_presets_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, special: bool) {
    let presets = app.all_task_filter_presets();
    let items: Vec<ListItem> = presets
        .iter()
        .map(|(name, query, builtin)| {
            ListItem::new(vec![
                Spans::from(Span::styled(
                    name.clone(),
                    ui_style::title_style(Accent::Tasks),
                )),
                Spans::from(Span::styled(query.clone(), ui_style::muted_style())),
                Spans::from(Span::styled(
                    if *builtin {
                        "Built-in preset".to_string()
                    } else {
                        "Saved preset".to_string()
                    },
                    ui_style::subtle_style(),
                )),
            ])
        })
        .collect();
    widgets::draw_list_popup(
        f,
        f.size(),
        PopupSize::Standard,
        Accent::Tasks,
        if special {
            "Special Task Presets (Enter apply, S save current, x delete saved)"
        } else {
            "Task Presets (Enter apply, S save current, x delete saved)"
        },
        items,
        Some(app.preset_selected),
    );
}

fn draw_save_task_preset_popup<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let active_query = if matches!(app.input_mode, InputMode::SavingSpecialPreset) {
        app.special_task_filter.as_str()
    } else {
        app.task_filter.as_str()
    };
    let heading = if matches!(app.input_mode, InputMode::SavingSpecialPreset) {
        "Save Special Task Preset"
    } else {
        "Save Task Preset"
    };
    let feedback = app
        .preset_form_message
        .clone()
        .unwrap_or_else(|| format!("Query: {active_query}"));
    widgets::draw_text_input_popup(
        f,
        size,
        PopupSize::Compact,
        Accent::Tasks,
        "Preset",
        heading,
        "Preset Name",
        app.preset_name_input.as_str(),
        &feedback,
        app.preset_form_message.is_some(),
    );
}
