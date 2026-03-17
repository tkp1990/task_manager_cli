use std::{error::Error, path::Path};

pub(crate) fn load_palette_history(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&content)?)
}

pub(crate) fn save_palette_history(path: &Path, commands: &[String]) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(commands)?;
    std::fs::write(path, content)?;
    Ok(())
}
