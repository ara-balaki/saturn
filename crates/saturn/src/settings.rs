//! What Saturn remembers between launches, in `~/Library/Application Support/Saturn/settings.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[expect(clippy::struct_excessive_bools, reason = "independent on/off switches, each its own JSON field")]
pub struct Settings {
    /// Projects opened before, most recent first, for the welcome screen.
    pub recent_projects: Vec<PathBuf>,
    /// The most recent project in settings written before `recent_projects` existed.
    #[serde(skip_serializing)]
    last_project: Option<PathBuf>,
    /// Tabs show the Claude session's name (or the terminal's title). Off: the folder, or a
    /// name typed by hand.
    pub name_tabs_after_sessions: bool,
    /// A slash command whose first word is a ticket (`/start abc-224`) names the session after it.
    pub rename_sessions_from_commands: bool,
    /// Saving a file runs the project's own formatter on it, when the project has one.
    pub format_on_save: bool,
    /// A notification when an agent needs you or finishes while Saturn is in the background.
    pub notifications: bool,
    pub appearance: AppearanceSetting,
}

/// Light or dark, or whichever macOS is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceSetting {
    #[default]
    System,
    Light,
    Dark,
}

impl AppearanceSetting {
    /// The appearance to draw in, given the one macOS is in.
    pub fn resolve(self, system: saturn_design::Appearance) -> saturn_design::Appearance {
        match self {
            Self::System => system,
            Self::Light => saturn_design::Appearance::Light,
            Self::Dark => saturn_design::Appearance::Dark,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recent_projects: Vec::new(),
            last_project: None,
            name_tabs_after_sessions: true,
            rename_sessions_from_commands: true,
            format_on_save: true,
            notifications: true,
            appearance: AppearanceSetting::System,
        }
    }
}

/// Where Saturn keeps what it remembers. Tests and spikes point it at their own folder.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SATURN_USER_DATA") {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/Saturn"))
}

fn file() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("settings.json"))
}

pub fn load() -> Settings {
    file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// How many projects the welcome screen offers.
const RECENT: usize = 8;

impl Settings {
    /// Recent projects that still exist, most recent first.
    pub fn recent(&self) -> Vec<PathBuf> {
        let mut recent = self.recent_projects.clone();
        if recent.is_empty() {
            recent.extend(self.last_project.clone());
        }
        recent.retain(|path| path.is_dir());
        recent
    }

    fn remember(&mut self, project: &Path) {
        let mut recent = self.recent();
        recent.retain(|path| path != project);
        recent.insert(0, project.to_owned());
        recent.truncate(RECENT);
        self.recent_projects = recent;
    }
}

pub fn remember_project(project: &Path) {
    let mut settings = load();
    settings.remember(project);
    save(&settings);
}

pub fn save(settings: &Settings) {
    let Some(path) = file() else { return };
    if let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_ok()
        && let Ok(text) = serde_json::to_string_pretty(settings)
    {
        let _ = std::fs::write(path, text);
    }
}

/// The folder to open straight away: `SATURN_PROJECT`, else the folder Saturn was started in from
/// a terminal. `None` (started from Finder, the Dock or Spotlight, which start apps in `/`) shows
/// the welcome screen instead.
pub fn startup_project() -> Option<PathBuf> {
    if let Some(project) = std::env::var_os("SATURN_PROJECT") {
        return Some(PathBuf::from(project));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    std::env::current_dir().ok().filter(|dir| dir != Path::new("/") && Some(dir) != home.as_ref())
}

#[cfg(test)]
mod tests {
    use saturn_design::Appearance;

    use super::*;

    #[test]
    fn appearance_follows_the_system_unless_chosen() {
        assert_eq!(AppearanceSetting::System.resolve(Appearance::Light), Appearance::Light);
        assert_eq!(AppearanceSetting::Dark.resolve(Appearance::Light), Appearance::Dark);
        assert_eq!(AppearanceSetting::Light.resolve(Appearance::Dark), Appearance::Light);
    }

    #[test]
    fn settings_saved_before_appearance_existed_still_load() {
        let settings: Settings = serde_json::from_str(r#"{"name_tabs_after_sessions": false}"#).expect("loads");
        assert_eq!(settings.appearance, AppearanceSetting::System);
        assert!(!settings.name_tabs_after_sessions);
    }

    #[test]
    fn recent_projects_keep_the_latest_first_without_duplicates() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        std::fs::create_dir_all(&a).expect("a");
        std::fs::create_dir_all(&b).expect("b");
        let mut settings = Settings::default();
        settings.remember(&a);
        settings.remember(&b);
        settings.remember(&a);
        assert_eq!(settings.recent(), [a.clone(), b]);
    }

    #[test]
    fn an_old_last_project_becomes_the_first_recent_one() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let text = format!(r#"{{"last_project": {:?}}}"#, dir.path());
        let settings: Settings = serde_json::from_str(&text).expect("loads");
        assert_eq!(settings.recent(), [dir.path().to_owned()]);
    }
}
