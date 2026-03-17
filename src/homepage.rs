mod data;
mod draw;
mod types;

use crossterm::event;
use std::io::Stdout;
use std::time::{Duration, Instant};
use tui::backend::CrosstermBackend;
use tui::Terminal;

use data::load_dashboard;
pub use types::AppTool;
use types::{handle_key, HomepageAction, HomepageDashboard};

/// Run the homepage (launcher) UI.
pub fn run_homepage(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tools = [
        AppTool::TaskManager,
        AppTool::Notes,
        AppTool::OneOnOneManager,
        AppTool::DelegationTracker,
        AppTool::DecisionLog,
    ];
    let mut selected = 0;
    let mut error_message: Option<String> = None;
    let mut dashboard = load_dashboard().unwrap_or_else(|err| {
        error_message = Some(err.to_string());
        HomepageDashboard {
            refreshed_at: "unavailable".to_string(),
            ..HomepageDashboard::default()
        }
    });
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            draw::draw_homepage(f, &tools, selected, &dashboard, error_message.as_deref());
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            let action = handle_key(event::read()?, &mut selected, tools.len());
            match action {
                HomepageAction::Continue => {}
                HomepageAction::Exit => break,
                HomepageAction::Refresh => match load_dashboard() {
                    Ok(new_dashboard) => {
                        dashboard = new_dashboard;
                        error_message = None;
                    }
                    Err(err) => error_message = Some(err.to_string()),
                },
                HomepageAction::Launch(mut tool) => {
                    error_message = tool.run(terminal).err().map(|err| err.to_string());
                    match load_dashboard() {
                        Ok(new_dashboard) => dashboard = new_dashboard,
                        Err(err) if error_message.is_none() => {
                            error_message = Some(err.to_string());
                        }
                        Err(_) => {}
                    }
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::data::{compact_text, scan_notes_tree};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(format!("task_manager_cli_homepage_{unique}"))
    }

    #[test]
    fn compact_text_truncates_long_values() {
        assert_eq!(compact_text("alpha beta gamma", 8), "alpha b…");
        assert_eq!(compact_text("short", 8), "short");
    }

    #[test]
    fn scan_notes_tree_counts_directories_and_files() -> Result<(), Box<dyn std::error::Error>> {
        let root = temp_dir("scan_notes");
        fs::create_dir_all(root.join("projects/alpha"))?;
        fs::write(root.join("projects/alpha/roadmap.md"), b"# roadmap")?;
        fs::write(root.join("inbox.md"), b"# inbox")?;

        let summary = scan_notes_tree(&root)?;

        assert_eq!(summary.file_count, 2);
        assert_eq!(summary.directory_count, 2);
        assert_eq!(summary.recent_files.len(), 2);
        assert!(summary
            .recent_files
            .iter()
            .any(|path| path.contains("roadmap.md")));

        let _ = fs::remove_dir_all(root);
        Ok(())
    }
}
