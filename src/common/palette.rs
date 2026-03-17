use std::{error::Error, fs, path::Path};

pub fn load_recent_commands(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(serde_json::from_str(&content)?)
}

pub fn save_recent_commands(path: &Path, commands: &[String]) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(commands)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn begin_palette<Mode: Copy>(
    query: &mut String,
    selected: &mut usize,
    return_mode: &mut Mode,
    input_mode: &mut Mode,
    palette_mode: Mode,
) {
    query.clear();
    *selected = 0;
    *return_mode = *input_mode;
    *input_mode = palette_mode;
}

pub fn close_palette<Mode: Copy>(
    query: &mut String,
    selected: &mut usize,
    return_mode: Mode,
    input_mode: &mut Mode,
) {
    query.clear();
    *selected = 0;
    *input_mode = return_mode;
}

pub fn record_recent_command(
    path: &Path,
    recent_commands: &mut Vec<String>,
    command_id: &str,
    limit: usize,
) -> Result<(), Box<dyn Error>> {
    recent_commands.retain(|item| item != command_id);
    recent_commands.insert(0, command_id.to_string());
    recent_commands.truncate(limit);
    save_recent_commands(path, recent_commands)?;
    Ok(())
}
