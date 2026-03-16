use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders};

#[derive(Clone, Copy)]
pub enum Accent {
    Primary,
    Tasks,
    Notes,
    LeadershipPeople,
    Delegation,
    Decisions,
}

#[derive(Clone, Copy)]
pub enum PopupSize {
    Compact,
    Standard,
    Wide,
    Tall,
    Full,
}

pub fn accent_color(accent: Accent) -> Color {
    match accent {
        Accent::Primary => Color::LightYellow,
        Accent::Tasks => Color::LightYellow,
        Accent::Notes => Color::LightCyan,
        Accent::LeadershipPeople => Color::LightGreen,
        Accent::Delegation => Color::LightBlue,
        Accent::Decisions => Color::LightMagenta,
    }
}

pub fn surface_block<'a>(title: &'a str, accent: Accent) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(accent_color(accent)))
}

pub fn shell_block<'a>(title: &'a str) -> Block<'a> {
    surface_block(title, Accent::Primary)
}

pub fn popup_block<'a>(title: &'a str, accent: Accent) -> Block<'a> {
    surface_block(title, accent)
}

pub fn selected_style() -> Style {
    Style::default()
        .bg(Color::Rgb(42, 74, 102))
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

pub fn focused_inline_style() -> Style {
    Style::default()
        .bg(Color::Rgb(95, 78, 19))
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

pub fn title_style(accent: Accent) -> Style {
    Style::default()
        .fg(accent_color(accent))
        .add_modifier(Modifier::BOLD)
}

pub fn body_style() -> Style {
    Style::default().fg(Color::White)
}

pub fn muted_style() -> Style {
    Style::default().fg(Color::Gray)
}

pub fn subtle_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn success_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

pub fn warning_style() -> Style {
    Style::default()
        .fg(Color::LightYellow)
        .add_modifier(Modifier::BOLD)
}

pub fn danger_style() -> Style {
    Style::default()
        .fg(Color::LightRed)
        .add_modifier(Modifier::BOLD)
}

pub fn info_style() -> Style {
    Style::default().fg(Color::LightCyan)
}

pub fn command_bar_block<'a>(title: &'a str) -> Block<'a> {
    shell_block(title)
}

pub fn shortcut_span(key: &str) -> Span<'static> {
    Span::styled(key.to_string(), title_style(Accent::Primary))
}

pub fn command_bar_spans(actions: &[(&str, &str)]) -> Spans<'static> {
    let mut spans = Vec::new();
    for (index, (key, label)) in actions.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(shortcut_span(key));
        spans.push(Span::raw(format!(" {label}")));
    }
    Spans::from(spans)
}

pub fn popup_rect(size: PopupSize, area: Rect) -> Rect {
    let (percent_x, percent_y) = match size {
        PopupSize::Compact => (50, 20),
        PopupSize::Standard => (55, 45),
        PopupSize::Wide => (62, 42),
        PopupSize::Tall => (60, 70),
        PopupSize::Full => (70, 70),
    };
    centered_rect(percent_x, percent_y, area)
}

pub fn badge(label: &str, accent: Accent) -> Span<'static> {
    Span::styled(
        format!("[{label}]"),
        Style::default()
            .fg(accent_color(accent))
            .add_modifier(Modifier::BOLD),
    )
}

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
