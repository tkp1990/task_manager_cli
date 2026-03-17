use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::error::Error;

use crate::leadership_tools::app::{App, InputMode, LinkedRecordKind};

pub(super) enum UiAction {
    Continue,
    Exit,
    OpenTask(i32),
    OpenNote(i32),
}

pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Result<UiAction, Box<dyn Error>> {
    match app.input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => return Ok(UiAction::Exit),
            KeyCode::Char('j') | KeyCode::Down => app.move_selection_down(),
            KeyCode::Char('k') | KeyCode::Up => app.move_selection_up(),
            KeyCode::Char('a') => app.begin_add(),
            KeyCode::Char('e') => app.begin_edit(),
            KeyCode::Char('d') => app.begin_delete(),
            KeyCode::Char('/') => app.begin_filter(),
            KeyCode::Char('o') => app.begin_link_browser()?,
            KeyCode::Char('x') => app.extract_sync_actions()?,
            KeyCode::Char('m') => app.complete_one_on_one()?,
            KeyCode::Char('r') => app.send_delegation_reminder()?,
            KeyCode::Char('v') => app.schedule_decision_review()?,
            KeyCode::Char('t') | KeyCode::Char('n') => app.quick_action()?,
            _ => {}
        },
        InputMode::Filtering => match key.code {
            KeyCode::Esc => app.clear_filter(),
            KeyCode::Enter => app.keep_filter(),
            KeyCode::Backspace => app.backspace(),
            KeyCode::Char(c) => app.append_char(c),
            _ => {}
        },
        InputMode::Editing => match key.code {
            KeyCode::Esc => app.cancel_modal(),
            KeyCode::Tab | KeyCode::Down => app.focus_next_field(),
            KeyCode::BackTab | KeyCode::Up => app.focus_prev_field(),
            KeyCode::Enter => app.advance_or_save()?,
            KeyCode::Backspace => app.backspace(),
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if c == 's' {
                    app.save_form()?;
                }
            }
            KeyCode::Char(c) => app.append_char(c),
            _ => {}
        },
        InputMode::DeleteConfirm => match key.code {
            KeyCode::Char('y') => app.delete_selected()?,
            KeyCode::Char('n') | KeyCode::Esc => app.cancel_modal(),
            _ => {}
        },
        InputMode::LinkedRecords => match key.code {
            KeyCode::Char('j') | KeyCode::Down => app.move_link_selection_down(),
            KeyCode::Char('k') | KeyCode::Up => app.move_link_selection_up(),
            KeyCode::Enter => {
                if let Some(link) = app.selected_linked_record().cloned() {
                    app.cancel_modal();
                    return Ok(match link.kind {
                        LinkedRecordKind::Task => UiAction::OpenTask(link.id),
                        LinkedRecordKind::Note => UiAction::OpenNote(link.id),
                    });
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => app.cancel_modal(),
            _ => {}
        },
    }
    Ok(UiAction::Continue)
}
