use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn temp_db_path(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}_{}_{}_{}",
        prefix,
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    );
    std::env::temp_dir().join(format!("task_manager_cli_{unique}.db"))
}

#[allow(dead_code)]
pub fn temp_notes_root(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}_{}_{}_{}",
        prefix,
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    );
    std::env::temp_dir().join(format!("task_manager_cli_notes_files_{unique}"))
}
