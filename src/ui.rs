use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, InputMode};

pub fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Define the layout.
    let size = f.size();
    // Split the screen into five sections:
    // 1. Topics (Tabs), 2. Tasks list, 3. Instructions, 4. Mode, 5. Logs.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),  // Topics Tabs
                Constraint::Min(5),     // Task list
                Constraint::Length(3),  // Instructions
                Constraint::Length(3),  // Mode indicator
                Constraint::Length(15), // Logs section
            ]
            .as_ref(),
        )
        .split(size);

    // --- TOPICS SECTION (Tabs) ---
    let titles: Vec<Spans> = app
        .topics
        .iter()
        .map(|t| Spans::from(Span::raw(&t.name)))
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.selected_topic)
        .block(Block::default().borders(Borders::ALL).title("Topics"))
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider(Span::raw("|"));
    f.render_widget(tabs, chunks[0]);

    // --- TASKS SECTION ---
    let items: Vec<ListItem> = app
        .tasks
        .iter()
        .map(|task| {
            // If task is expanded, show extra details; otherwise, only show the task name.
            let lines = if app.expanded.contains(&task.id) {
                vec![
                    Spans::from(Span::raw(format!("{}", task.description))),
                    Spans::from(Span::styled(
                        format!(
                            "ID: {} | Completed: {} | Favourite: {} | Created: {} | Updated: {}",
                            task.id,
                            if task.completed { "Yes" } else { "No" },
                            if task.favourite { "Yes" } else { "No" },
                            task.created_at,
                            task.updated_at
                        ),
                        Style::default().fg(Color::Gray),
                    )),
                ]
            } else {
                vec![Spans::from(Span::raw(format!("{}", task.description)))]
            };
            ListItem::new(lines)
        })
        .collect();

    let tasks_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Tasks"))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected));
    f.render_stateful_widget(tasks_list, chunks[1], &mut list_state);

    // Instructions or input area.
    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("a", Style::default().fg(Color::Yellow)),
                Span::raw(" to add, "),
                Span::styled("t", Style::default().fg(Color::Yellow)),
                Span::raw(" to toggle, "),
                Span::styled("f", Style::default().fg(Color::Yellow)),
                Span::raw(" to toggle favourite, "),
                Span::styled("e", Style::default().fg(Color::Yellow)),
                Span::raw(" to edit selected task, "),
                Span::styled("d", Style::default().fg(Color::Yellow)),
                Span::raw(" to delete, Up/Down or j/k to navigate, "),
                Span::styled("N", Style::default().fg(Color::Yellow)),
                Span::raw(" to add topic, "),
                Span::styled("X", Style::default().fg(Color::Yellow)),
                Span::raw(" to delete topic, Enter to expand/collapse a task, "),
                Span::raw(" PageUp/PageDown to scroll logs, "),
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(" to quit."),
            ],
            Style::default(),
        ),
        InputMode::AddingTask => (
            vec![
                Span::raw("Enter task description (Press Enter to add, Esc to cancel): "),
                Span::raw(&app.input),
            ],
            Style::default().fg(Color::Green),
        ),
        InputMode::AddingTopic => (
            vec![
                Span::raw("Enter topic name (Press Enter to add, Esc to cancel): "),
                Span::raw(&app.input),
            ],
            Style::default().fg(Color::Green),
        ),
        InputMode::EditingTask => (
            vec![
                Span::raw("Edit task description (Press Enter to save, Esc to cancel):"),
                Span::raw(&app.input),
            ],
            Style::default().fg(Color::Green),
        ),
    };

    let help_message = Paragraph::new(Spans::from(msg))
        .style(style)
        .block(Block::default().borders(Borders::ALL).title("Instructions"));

    f.render_widget(help_message, chunks[2]);

    // (Optional) Show current mode at the bottom.
    let mode_text = match app.input_mode {
        InputMode::Normal => "Normal Mode",
        InputMode::AddingTask => "Add Mode",
        InputMode::EditingTask => "Editing Mode",
        InputMode::AddingTopic => "Adding Topic",
    };
    let mode =
        Paragraph::new(mode_text).block(Block::default().borders(Borders::ALL).title("Mode"));
    f.render_widget(mode, chunks[3]);

    // --- LOGS SECTION ---
    // Determine how many lines can be shown.
    let log_area_height = chunks[4].height as usize;
    let total_logs = app.logs.len();
    // Compute the starting index, ensuring we don't underflow.
    let start = if total_logs > log_area_height + app.log_offset {
        total_logs - log_area_height - app.log_offset
    } else {
        0
    };
    let visible_logs: Vec<ListItem> = app.logs[start..]
        .iter()
        .map(|line| ListItem::new(Span::raw(line)))
        .collect();
    let logs_list =
        List::new(visible_logs).block(Block::default().borders(Borders::ALL).title("Logs"));
    f.render_widget(logs_list, chunks[4]);
}
