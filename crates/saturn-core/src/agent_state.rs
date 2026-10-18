//! What a terminal tab shows about the program in it, and how its name is read from the terminal
//! title. Matches `AgentStatus` in the design system.

use crate::program_status::ProgramStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentState {
    /// A plain shell; no agent reported anything.
    #[default]
    Shell,
    Idle,
    Working,
    NeedsYou,
    YourTurn,
    Error,
}

impl AgentState {
    /// States that ask for the user, and so light a pane up until it has been looked at.
    pub fn asks_for_you(self) -> bool {
        matches!(self, Self::NeedsYou | Self::YourTurn | Self::Error)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Shell => "Shell",
            Self::Idle => "Idle",
            Self::Working => "Working",
            Self::NeedsYou => "Needs you",
            Self::YourTurn => "Your turn",
            Self::Error => "Error",
        }
    }

    /// Maps Claude Code's OSC 7501 report to a tab state. Unknown future states read as working.
    pub fn from_program_status(status: &ProgramStatus) -> Self {
        match status.state.as_str() {
            "clear" => Self::Shell,
            "idle" => Self::Idle,
            "blocked" => Self::NeedsYou,
            "done" => Self::YourTurn,
            "error" => Self::Error,
            _ => Self::Working,
        }
    }
}

/// Spinner glyphs Claude Code may put in front of its terminal title.
fn is_spinner(c: char) -> bool {
    matches!(c, '·' | '✢' | '✳' | '✶' | '✻' | '✽' | '*') || ('\u{2801}'..='\u{28ff}').contains(&c)
}

/// A terminal title as a short tab name: Claude's session name, or the shell's folder for titles
/// like oh-my-zsh's `user@host: ~/some/path`.
pub fn tab_title(title: &str) -> Option<String> {
    let mut trimmed = title.trim();
    if let Some(first) = trimmed.chars().next()
        && is_spinner(first)
    {
        trimmed = trimmed[first.len_utf8()..].trim_start();
    }
    if trimmed.is_empty() {
        return None;
    }
    if let Some((user_host, path)) = trimmed.split_once(':')
        && let Some((user, host)) = user_host.split_once('@')
        && !user.is_empty()
        && !host.is_empty()
        && !user_host.contains(char::is_whitespace)
    {
        let path = path.trim().trim_end_matches('/');
        let folder = path.rsplit('/').next().filter(|f| !f.is_empty()).unwrap_or(path);
        return (!folder.is_empty()).then(|| folder.to_owned());
    }
    Some(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str) -> AgentState {
        AgentState::from_program_status(&ProgramStatus { state: name.into(), ..Default::default() })
    }

    #[test]
    fn maps_program_status_to_tab_states() {
        assert_eq!(state("idle"), AgentState::Idle);
        assert_eq!(state("working"), AgentState::Working);
        assert_eq!(state("blocked"), AgentState::NeedsYou);
        assert_eq!(state("done"), AgentState::YourTurn);
        assert_eq!(state("clear"), AgentState::Shell);
        assert_eq!(state("error"), AgentState::Error);
        assert_eq!(state("something-new"), AgentState::Working);
    }

    #[test]
    fn keeps_a_claude_session_name() {
        assert_eq!(tab_title("ABC-236").as_deref(), Some("ABC-236"));
    }

    #[test]
    fn drops_a_leading_spinner_glyph() {
        assert_eq!(tab_title("✳ ABC-236").as_deref(), Some("ABC-236"));
    }

    #[test]
    fn turns_a_shell_prompt_title_into_its_folder() {
        assert_eq!(tab_title("jonasarden@Mac: ~/Documents/Code/Acme/acme-web").as_deref(), Some("acme-web"));
        assert_eq!(tab_title("jonasarden@Mac:~").as_deref(), Some("~"));
    }

    #[test]
    fn gives_nothing_for_an_empty_title() {
        assert_eq!(tab_title("   "), None);
    }
}
