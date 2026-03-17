use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::ui_style::{self, Accent, PopupSize};

pub fn draw_confirmation_popup<B: Backend>(
    f: &mut Frame<B>,
    size: Rect,
    accent: Accent,
    shell_title: &str,
    heading: &str,
    message: &str,
    controls: &str,
) {
    let popup_area = ui_style::popup_rect(PopupSize::Compact, size);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(3),
        ])
        .split(popup_area);

    let title = Paragraph::new(heading)
        .style(ui_style::title_style(accent))
        .alignment(Alignment::Center)
        .block(ui_style::popup_block(shell_title, accent));
    f.render_widget(title, layout[0]);

    let body = Paragraph::new(message)
        .style(ui_style::danger_style())
        .alignment(Alignment::Center)
        .block(ui_style::popup_block("Confirmation", accent));
    f.render_widget(body, layout[1]);

    let instructions = Paragraph::new(controls)
        .style(ui_style::info_style())
        .alignment(Alignment::Center)
        .block(ui_style::popup_block("Controls", accent));
    f.render_widget(instructions, layout[2]);
}

pub fn draw_text_input_popup<B: Backend>(
    f: &mut Frame<B>,
    size: Rect,
    popup_size: PopupSize,
    accent: Accent,
    shell_title: &str,
    heading: &str,
    input_label: &str,
    input_value: &str,
    feedback: &str,
    feedback_is_error: bool,
) {
    let popup_area = ui_style::popup_rect(popup_size, size);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(popup_area);

    let title = Paragraph::new(heading)
        .style(ui_style::title_style(accent))
        .block(ui_style::popup_block(shell_title, accent));
    f.render_widget(title, layout[0]);

    let input = Paragraph::new(input_value)
        .style(ui_style::body_style())
        .block(ui_style::popup_block(input_label, accent));
    f.render_widget(input, layout[1]);
    f.set_cursor(layout[1].x + input_value.len() as u16 + 1, layout[1].y + 1);

    let feedback = Paragraph::new(feedback)
        .style(if feedback_is_error {
            ui_style::danger_style()
        } else {
            ui_style::subtle_style()
        })
        .block(ui_style::popup_block("Feedback", accent));
    f.render_widget(feedback, layout[2]);
}

pub fn draw_list_popup<B: Backend>(
    f: &mut Frame<B>,
    size: Rect,
    popup_size: PopupSize,
    accent: Accent,
    title: &str,
    items: Vec<ListItem>,
    selected: Option<usize>,
) {
    let popup_area = ui_style::popup_rect(popup_size, size);
    f.render_widget(Clear, popup_area);

    let list = List::new(items)
        .block(ui_style::popup_block(title, accent))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");

    let mut state = ListState::default();
    state.select(selected);
    f.render_stateful_widget(list, popup_area, &mut state);
}
