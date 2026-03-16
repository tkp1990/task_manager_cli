use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedFilterPreset {
    pub name: String,
    pub query: String,
}

pub fn load_presets(path: &Path) -> Result<Vec<SavedFilterPreset>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(serde_json::from_str(&content)?)
}

pub fn save_presets(path: &Path, presets: &[SavedFilterPreset]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(presets)?;
    fs::write(path, content)?;
    Ok(())
}
