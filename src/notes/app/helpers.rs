use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::{
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use super::{FileMetadata, NoteReference, SavedFileShortcut, TemplateDefinition};

pub(super) fn render_preview_content(path: &Path, content: &str) -> String {
    let is_markdown = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false);

    if is_markdown {
        render_markdown_preview(content)
    } else {
        content.to_string()
    }
}

pub(super) fn load_file_shortcuts(path: &Path) -> Result<Vec<SavedFileShortcut>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(serde_json::from_str(&content)?)
}

pub(super) fn save_file_shortcuts(
    path: &Path,
    shortcuts: &[SavedFileShortcut],
) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(shortcuts)?;
    fs::write(path, content)?;
    Ok(())
}

pub(super) fn load_palette_history(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(serde_json::from_str(&content)?)
}

pub(super) fn save_palette_history(path: &Path, commands: &[String]) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(commands)?;
    fs::write(path, content)?;
    Ok(())
}

pub(super) fn load_custom_templates(dir: &Path) -> Result<Vec<TemplateDefinition>, Box<dyn Error>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut templates = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_file() {
            let content = fs::read_to_string(&path)?;
            let name = path
                .file_stem()
                .and_then(OsStr::to_str)
                .unwrap_or("template")
                .to_string();
            templates.push(TemplateDefinition {
                name,
                content,
                is_custom: true,
            });
        }
    }
    templates.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(templates)
}

pub(super) fn copy_path_recursive(source: &Path, target: &Path) -> Result<(), Box<dyn Error>> {
    if source.is_dir() {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let child_source = entry.path();
            let child_target = target.join(entry.file_name());
            copy_path_recursive(&child_source, &child_target)?;
        }
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
    }
    Ok(())
}

pub(super) fn parse_file_metadata(content: &str) -> FileMetadata {
    let mut metadata = FileMetadata::default();
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return metadata;
    }

    let frontmatter_lines = lines
        .by_ref()
        .take_while(|line| *line != "---")
        .collect::<Vec<_>>();

    let mut i = 0;
    while i < frontmatter_lines.len() {
        let line = frontmatter_lines[i].trim();
        if let Some(rest) = line.strip_prefix("title:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                metadata.title = Some(value.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("tags:") {
            let rest = rest.trim();
            if rest.starts_with('[') && rest.ends_with(']') {
                metadata.tags.extend(
                    rest.trim_matches(|c| c == '[' || c == ']')
                        .split(',')
                        .map(|tag| tag.trim().trim_matches('"').trim_matches('\''))
                        .filter(|tag| !tag.is_empty())
                        .map(|tag| tag.to_string()),
                );
            } else if !rest.is_empty() {
                metadata.tags.extend(
                    rest.split(',')
                        .map(|tag| tag.trim().trim_matches('"').trim_matches('\''))
                        .filter(|tag| !tag.is_empty())
                        .map(|tag| tag.to_string()),
                );
            } else {
                i += 1;
                while i < frontmatter_lines.len() {
                    let tag_line = frontmatter_lines[i].trim();
                    if let Some(tag) = tag_line.strip_prefix("- ") {
                        let tag = tag.trim().trim_matches('"').trim_matches('\'');
                        if !tag.is_empty() {
                            metadata.tags.push(tag.to_string());
                        }
                        i += 1;
                    } else {
                        i = i.saturating_sub(1);
                        break;
                    }
                }
            }
        }
        i += 1;
    }

    metadata.tags.sort();
    metadata.tags.dedup();
    metadata
}

pub(super) fn line_count(content: &str) -> usize {
    if content.is_empty() {
        1
    } else {
        content.lines().count().max(1)
    }
}

pub(super) fn extract_note_references(
    notes_root: &Path,
    source_path: &Path,
    content: &str,
) -> Vec<NoteReference> {
    let mut references = Vec::new();

    let mut remainder = content;
    while let Some(start) = remainder.find("[[") {
        let after_start = &remainder[start + 2..];
        if let Some(end) = after_start.find("]]") {
            let raw = after_start[..end].trim();
            if let Some(path) = resolve_reference_path(notes_root, source_path, raw) {
                references.push(NoteReference {
                    label: raw.to_string(),
                    path,
                });
            }
            remainder = &after_start[end + 2..];
        } else {
            break;
        }
    }

    let parser = Parser::new(content);
    for event in parser {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            let raw = dest_url.to_string();
            if let Some(path) = resolve_reference_path(notes_root, source_path, &raw) {
                references.push(NoteReference { label: raw, path });
            }
        }
    }

    references.sort_by(|left, right| left.label.cmp(&right.label));
    references.dedup_by(|left, right| left.path == right.path);
    references
}

fn resolve_reference_path(notes_root: &Path, source_path: &Path, raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with('#')
    {
        return None;
    }

    let candidate = if trimmed.starts_with('/') {
        notes_root.join(trimmed.trim_start_matches('/'))
    } else if trimmed.contains('/') || trimmed.ends_with(".md") || trimmed.ends_with(".markdown") {
        source_path.parent().unwrap_or(notes_root).join(trimmed)
    } else {
        notes_root.join(format!("{trimmed}.md"))
    };

    let normalized = normalize_note_path(&candidate);
    if normalized.starts_with(notes_root) && normalized.exists() {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_note_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

pub fn format_file_size(size_bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if size_bytes >= MB {
        format!("{:.1} MB", size_bytes as f64 / MB as f64)
    } else if size_bytes >= KB {
        format!("{:.1} KB", size_bytes as f64 / KB as f64)
    } else {
        format!("{size_bytes} B")
    }
}

pub(super) fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }

    text.char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| text.len())
}

fn render_markdown_preview(content: &str) -> String {
    let mut output = String::new();
    let mut bullet_depth = 0usize;
    let mut current_heading: Option<HeadingLevel> = None;
    let mut in_code_block = false;

    for event in Parser::new(content) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                output.push('\n');
                current_heading = None;
            }
            Event::Start(Tag::List(_)) => {
                bullet_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                bullet_depth = bullet_depth.saturating_sub(1);
                output.push('\n');
            }
            Event::Start(Tag::Item) => {
                output.push_str(&"  ".repeat(bullet_depth.saturating_sub(1)));
                output.push_str("- ");
            }
            Event::End(TagEnd::Item) => output.push('\n'),
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                output.push_str("\n```text\n");
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                output.push_str("\n```\n");
            }
            Event::SoftBreak | Event::HardBreak => output.push('\n'),
            Event::Rule => output.push_str("\n----------------\n"),
            Event::Text(text) => {
                if let Some(level) = current_heading {
                    let prefix = match level {
                        HeadingLevel::H1 => "# ",
                        HeadingLevel::H2 => "## ",
                        HeadingLevel::H3 => "### ",
                        HeadingLevel::H4 => "#### ",
                        HeadingLevel::H5 => "##### ",
                        HeadingLevel::H6 => "###### ",
                    };
                    if output.is_empty() || output.ends_with('\n') {
                        output.push_str(prefix);
                    }
                }
                output.push_str(&text);
            }
            Event::Code(text) => {
                output.push('`');
                output.push_str(&text);
                output.push('`');
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                output.push('[');
                output.push_str(&dest_url);
                output.push_str("] ");
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                if in_code_block {
                    output.push_str(&html);
                }
            }
            _ => {}
        }
    }

    output.lines().take(200).collect::<Vec<_>>().join("\n")
}

pub(super) fn fuzzy_matches(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let mut query_chars = query.chars();
    let mut current = query_chars.next();
    for ch in text.chars() {
        if let Some(expected) = current {
            if ch == expected {
                current = query_chars.next();
                if current.is_none() {
                    return true;
                }
            }
        } else {
            return true;
        }
    }
    current.is_none()
}

pub(super) fn shell_quote(path: &Path) -> String {
    let escaped = path.display().to_string().replace('\'', "'\\''");
    format!("'{escaped}'")
}
