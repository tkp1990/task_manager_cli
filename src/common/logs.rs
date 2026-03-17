use chrono::Local;

pub fn push_timestamped_log(
    logs: &mut Vec<String>,
    log_offset: &mut usize,
    level: &str,
    message: &str,
) {
    let now = Local::now();
    let entry = format!(
        "{} [{}] {}",
        now.format("%Y-%m-%d %H:%M:%S"),
        level,
        message
    );
    logs.push(entry);
    *log_offset = 0;
}
