use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, InputMode};

/// Helper function to create a centered rectangle with the given percentage size.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);
    let vertical = popup_layout[1];
    let popup_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(vertical);
    popup_layout[1]
}

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

    // --- INSTRUCTIONS SECTION ---
    // In Normal mode, only show minimal instructions.
    let help_hint = Spans::from(vec![
        Span::raw("Press "),
        Span::styled("H", Style::default().fg(Color::Yellow)),
        Span::raw(" for help."),
    ]);
    let input_msg = match app.input_mode {
        InputMode::AddingTask => Spans::from(vec![
            Span::raw("Enter task description (Press Enter to add, Esc to cancel): "),
            Span::raw(&app.input),
        ]),
        InputMode::AddingTopic => Spans::from(vec![
            Span::raw("Enter topic name (Press Enter to add, Esc to cancel): "),
            Span::raw(&app.input),
        ]),
        InputMode::EditingTask => Spans::from(vec![
            Span::raw("Edit task description (Press Enter to save, Esc to cancel): "),
            Span::raw(&app.input),
        ]),
        _ => help_hint,
    };
    let help_message = Paragraph::new(input_msg)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Instructions"));
    f.render_widget(help_message, chunks[2]);

    // (Optional) Show current mode at the bottom.
    let mode_text = match app.input_mode {
        InputMode::Normal => "Normal Mode",
        InputMode::AddingTask => "Add Mode",
        InputMode::EditingTask => "Editing Mode",
        InputMode::AddingTopic => "Adding Topic",
        InputMode::Help => "Viewing Help",
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

    // --- HELP POPUP (if enabled) ---
    if app.show_help {
        let help_text = vec![
            Spans::from(Span::styled("Help - Available Operations", Style::default().add_modifier(Modifier::BOLD))),
            Spans::from(""),
            Spans::from(Span::raw("Add Task: Press 'a' to add a new task (in a non-Favourites topic). Type the description and press Enter.")),
            Spans::from(Span::raw("Add Task: Press 'e' to edit a task. Edit the description and press Enter.")),
            Spans::from(Span::raw("Toggle Complete: Press 't' to mark a task complete/incomplete.")),
            Spans::from(Span::raw("Toggle Favourite: Press 'f' to mark/unmark a task as favourite.")),
            Spans::from(Span::raw("Delete Task: Press 'd' to delete the selected task.")),
            Spans::from(Span::raw("Expand/Collapse Task: Press Enter on a task to show/hide details.")),
            Spans::from(Span::raw("Navigating Tasks: Use Up/Down arrow keys or j/k to change topics.")),
            Spans::from(Span::raw("Switch Topics: Use Left/Right arrow keys or h/l to change topics.")),
            Spans::from(Span::raw("Add Topic: Press 'N' to add a new topic. Then enter the topic name and press Enter.")),
            Spans::from(Span::raw("Delete Topic: Press 'X' to delete the current topic (Favourites is protected).")),
            Spans::from(Span::raw("Scroll Logs: Use PageUp/PageDown to scroll through logs.")),
            Spans::from(Span::raw("Toggle Help: Press 'H' to hide this help window.")),
            Spans::from(Span::raw("Quit: Press 'q' to exit the application.")),
        ];
        let help_paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(tui::widgets::Wrap { trim: true });
        let area = centered_rect(60, 70, size);
        // Clear the background behind the popup
        f.render_widget(Clear, area);
        f.render_widget(help_paragraph, area);
    }
}
