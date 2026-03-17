use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Span, Spans},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::ui_style::{self, Accent, PopupSize};

#[derive(Clone, Copy)]
pub struct PaletteCommand {
    pub id: &'static str,
    pub shortcut: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub keywords: &'static str,
}

pub fn visible_commands(
    commands: impl IntoIterator<Item = PaletteCommand>,
    query: &str,
    recent_commands: &[String],
) -> Vec<PaletteCommand> {
    let normalized_query = query.trim().to_lowercase();
    let mut visible = commands
        .into_iter()
        .filter(|command| palette_matches(command, query))
        .collect::<Vec<_>>();

    visible.sort_by_key(|command| {
        let recent_rank = recent_commands
            .iter()
            .position(|item| item == command.id)
            .unwrap_or(usize::MAX);
        let label = command.label.to_lowercase();
        let keyword_hit = command.keywords.to_lowercase().contains(&normalized_query);
        let prefix = !normalized_query.is_empty() && label.starts_with(&normalized_query);
        (
            recent_rank,
            !prefix,
            !keyword_hit,
            command.group,
            command.label,
        )
    });

    visible
}

pub fn draw_popup<B: Backend>(
    f: &mut Frame<B>,
    size: Rect,
    query: &str,
    selected: usize,
    commands: &[PaletteCommand],
    accent: Accent,
) {
    let popup_area = ui_style::popup_rect(PopupSize::Wide, size);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(popup_area);

    let input = Paragraph::new(query)
        .style(ui_style::body_style())
        .block(ui_style::popup_block("Command Palette", accent));
    f.render_widget(input, layout[0]);
    f.set_cursor(layout[0].x + query.len() as u16 + 1, layout[0].y + 1);

    let items: Vec<ListItem> = if commands.is_empty() {
        vec![ListItem::new(vec![Spans::from(Span::styled(
            "No matching commands.",
            ui_style::muted_style(),
        ))])]
    } else {
        commands
            .iter()
            .map(|command| {
                ListItem::new(vec![
                    Spans::from(vec![
                        Span::styled(command.group, ui_style::muted_style()),
                        Span::raw("  "),
                        Span::styled(command.label, ui_style::title_style(accent)),
                        Span::raw("  "),
                        Span::styled(command.shortcut, ui_style::info_style()),
                    ]),
                    Spans::from(Span::styled(command.description, ui_style::muted_style())),
                ])
            })
            .collect()
    };

    let list = List::new(items)
        .block(ui_style::popup_block("Matches", accent))
        .highlight_style(ui_style::selected_style())
        .highlight_symbol("=> ");
    let mut state = ListState::default();
    if !commands.is_empty() {
        state.select(Some(selected.min(commands.len() - 1)));
    }
    f.render_stateful_widget(list, layout[1], &mut state);

    let footer = Paragraph::new(vec![
        ui_style::command_bar_spans(&[("Enter", "run"), ("j/k", "move"), ("Esc", "close")]),
        Spans::from(Span::styled(
            "Recent commands rank first when the query is empty or ambiguous.",
            ui_style::muted_style(),
        )),
    ])
    .style(ui_style::info_style())
    .block(ui_style::popup_block("Palette Controls", accent));
    f.render_widget(footer, layout[2]);
}

fn palette_matches(command: &PaletteCommand, query: &str) -> bool {
    let trimmed = query.trim().to_lowercase();
    trimmed.is_empty()
        || command.label.to_lowercase().contains(&trimmed)
        || command.description.to_lowercase().contains(&trimmed)
        || command.keywords.to_lowercase().contains(&trimmed)
}
