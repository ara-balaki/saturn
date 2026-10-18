//! The grid as it was, per project, so Saturn opens where it left off: the shape of the grid, and
//! for each tab its folder, the name typed for it, and the Claude session it was running.
//! Kept in `~/Library/Application Support/Saturn/sessions.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use saturn_core::layout::Shape;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedGrid {
    pub shape: Shape,
    /// One per tab, in the order the shape lists them.
    pub tabs: Vec<SavedTab>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTab {
    pub cwd: PathBuf,
    /// A name typed by hand for the tab.
    #[serde(default)]
    pub name: Option<String>,
    /// The Claude session that was running in it, offered for resuming.
    #[serde(default)]
    pub claude_session: Option<String>,
}

fn file() -> Option<PathBuf> {
    crate::settings::data_dir().map(|dir| dir.join("sessions.json"))
}

fn read_all() -> HashMap<PathBuf, SavedGrid> {
    file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// The grid saved for `project`, if any.
pub fn load(project: &Path) -> Option<SavedGrid> {
    read_all().remove(project).filter(|grid| !grid.tabs.is_empty() && grid.shape.tab_count() == grid.tabs.len())
}

/// Saves `project`'s grid, or forgets it when `grid` is `None` (the last tab closed).
pub fn save(project: &Path, grid: Option<SavedGrid>) {
    let Some(path) = file() else { return };
    let mut all = read_all();
    match grid {
        Some(grid) => all.insert(project.to_owned(), grid),
        None => all.remove(project),
    };
    if let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_ok()
        && let Ok(text) = serde_json::to_string_pretty(&all)
    {
        let _ = std::fs::write(path, text);
    }
}

/// What to type into a restored tab to resume its Claude session: typed, not run, so resuming is
/// one Enter and never a surprise.
pub fn resume_command(session: &str) -> Option<String> {
    // Session ids are UUIDs; anything else is not typed into a shell.
    session.chars().all(|c| c.is_ascii_hexdigit() || c == '-').then(|| format!("claude --resume {session}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_uuid_like_sessions_are_offered() {
        assert_eq!(
            resume_command("0b6f7c2e-1a2b-4c3d-9e8f-001122334455").as_deref(),
            Some("claude --resume 0b6f7c2e-1a2b-4c3d-9e8f-001122334455")
        );
        assert_eq!(resume_command("x; rm -rf ~"), None);
    }
}
