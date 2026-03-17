use std::{error::Error, path::Path};

pub(crate) fn load_palette_history(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    crate::common::palette::load_recent_commands(path)
}
