//! What Saturn remembers between launches, in `~/Library/Application Support/Saturn/settings.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[expect(clippy::struct_excessive_bools, reason = "independent on/off switches, each its own JSON field")]
pub struct Settings {
    pub last_project: Option<PathBuf>,
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

pub fn remember_project(project: &Path) {
    let mut settings = load();
    settings.last_project = Some(project.to_owned());
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

/// The folder to open: `SATURN_PROJECT`, else the folder Saturn was started in (when started from
/// a terminal), else the last project, else home.
pub fn startup_project() -> PathBuf {
    if let Some(project) = std::env::var_os("SATURN_PROJECT") {
        return PathBuf::from(project);
    }
    let started_in = std::env::current_dir().ok().filter(|dir| dir != Path::new("/"));
    started_in
        .or_else(|| load().last_project.filter(|path| path.is_dir()))
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/"))
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
}
