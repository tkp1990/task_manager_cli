use slog::{o, Drain, Logger};
use slog_async;
use slog_envlogger;
use slog_envlogger::LogBuilder;
use slog_json;
use slog_term;
use std::fs::{create_dir_all, OpenOptions};
use std::path::Path;
use std::sync::OnceLock;

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn init_logger() -> &'static Logger {
    LOGGER.get_or_init(|| {
        // ✅ Make sure logs/ exists
        let log_dir = "logs";
        if !Path::new(log_dir).exists() {
            create_dir_all(log_dir).expect("Failed to create logs directory");
        }

        // Terminal drain
        let decorator = slog_term::TermDecorator::new().build();
        let term_drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let term_drain = slog_async::Async::new(term_drain).build().fuse();

        // File drain (JSON)
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open("logs/app.log")
            .expect("Cannot open log file");
        let file_drain = slog_json::Json::default(file).fuse();
        let file_drain = slog_async::Async::new(file_drain).build().fuse();

        // Combine and wrap with envlogger
        let combined_drain = slog::Duplicate::new(term_drain, file_drain).fuse();

        let env_drain = LogBuilder::new(combined_drain)
            .parse(&std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
            .build()
            .fuse();
        // Root logger
        Logger::root(env_drain, o!("version" => env!("CARGO_PKG_VERSION")))
    })
}

pub fn get_logger() -> &'static Logger {
    LOGGER
        .get()
        .expect("Logger not initialized. Call init_logger() first.")
}
