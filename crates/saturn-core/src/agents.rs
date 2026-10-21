//! The coding agents Saturn runs in its panes, Claude Code and Codex, and what differs between them:
//! the program, how it starts with a first message, how a session resumes, where it keeps its
//! sessions. Which of them a Mac has is found, not set: on the login `PATH`, and logged in.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    /// Its name, as people know it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    /// The program that runs it, as typed and as the process is named.
    #[must_use]
    pub fn program(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// The agent a process is, by its name.
    #[must_use]
    pub fn of_program(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|agent| agent.program() == name.trim_start_matches('-'))
    }

    /// The agent a command starts, by its first word.
    #[must_use]
    pub fn of_command(command: &str) -> Option<Self> {
        command.split_whitespace().next().and_then(Self::of_program)
    }

    /// The command that starts it in a pane, with `prompt` as its first message when there is one,
    /// quoted for the shell by `quote`.
    #[must_use]
    pub fn start(self, prompt: Option<&str>, quote: impl Fn(&str) -> String) -> String {
        match prompt {
            Some(prompt) => format!("{} {}", self.program(), quote(prompt)),
            None => self.program().to_owned(),
        }
    }

    /// The command that resumes `session` in the folder it ran in.
    #[must_use]
    pub fn resume(self, session: &str) -> String {
        match self {
            Self::Claude => format!("claude --resume {session}"),
            Self::Codex => format!("codex resume {session}"),
        }
    }

    /// What starting it on a ticket runs, before a project chooses its own: Claude Code names its
    /// session after the ticket (`-n`); Codex has no such flag.
    #[must_use]
    pub fn ticket_command(self) -> &'static str {
        match self {
            Self::Claude => "claude -n {ticket}",
            Self::Codex => "codex",
        }
    }

    /// Where it keeps its settings and sessions: `CLAUDE_CONFIG_DIR` or `CODEX_HOME` when set, else
    /// `~/.claude` or `~/.codex`.
    #[must_use]
    pub fn home(self) -> Option<PathBuf> {
        let (variable, folder) = match self {
            Self::Claude => ("CLAUDE_CONFIG_DIR", ".claude"),
            Self::Codex => ("CODEX_HOME", ".codex"),
        };
        std::env::var_os(variable)
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(folder)))
    }

    /// The command that says whether it's logged in, without starting a session.
    fn login_check(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["auth", "status"],
            Self::Codex => &["login", "status"],
        }
    }

    /// Whether its login check says it's logged in: Claude Code answers JSON with `loggedIn`, Codex
    /// a sentence (`Logged in using ChatGPT`) and success.
    #[must_use]
    pub fn logged_in_from(self, output: &str, success: bool) -> bool {
        match self {
            // The JSON comes first; a warning on stderr may follow it.
            Self::Claude => serde_json::Deserializer::from_str(output)
                .into_iter::<serde_json::Value>()
                .next()
                .and_then(Result::ok)
                .and_then(|status| status.get("loggedIn")?.as_bool())
                .unwrap_or(false),
            Self::Codex => success && output.contains("Logged in"),
        }
    }
}

/// What Saturn found of an agent on this Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub agent: Agent,
    /// Its program is on the login `PATH`.
    pub installed: bool,
    /// It says it's logged in, so it can run.
    pub logged_in: bool,
}

impl Found {
    /// Installed and logged in: an agent Saturn can start.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.installed && self.logged_in
    }
}

/// Finds each agent with `path` as the `PATH` (the login shell's: Saturn started from Finder has
/// little of its own). Runs their login checks, which take a moment: off the main thread.
#[must_use]
pub fn detect(path: &str) -> Vec<Found> {
    Agent::ALL
        .into_iter()
        .map(|agent| {
            let output = Command::new(agent.program()).args(agent.login_check()).env("PATH", path).output();
            match output {
                Ok(output) => {
                    // Codex says it on stderr, Claude Code on stdout.
                    let said = [output.stdout, output.stderr].concat();
                    Found {
                        agent,
                        installed: true,
                        logged_in: agent.logged_in_from(&String::from_utf8_lossy(&said), output.status.success()),
                    }
                }
                Err(_) => Found { agent, installed: false, logged_in: false },
            }
        })
        .collect()
}

/// The agent a new pane starts: the one chosen last when it's ready, else the first ready one, else
/// Claude Code (whose missing program the shell then reports).
#[must_use]
pub fn pick(found: &[Found], last: Option<Agent>) -> Agent {
    let ready = |agent: Agent| found.iter().any(|found| found.agent == agent && found.ready());
    last.filter(|agent| ready(*agent))
        .or_else(|| Agent::ALL.into_iter().find(|agent| ready(*agent)))
        .unwrap_or(Agent::Claude)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote(text: &str) -> String {
        format!("'{text}'")
    }

    #[test]
    fn starts_and_resumes_each_agent() {
        assert_eq!(Agent::Claude.start(Some("Fix the login"), quote), "claude 'Fix the login'");
        assert_eq!(Agent::Codex.start(Some("Fix the login"), quote), "codex 'Fix the login'");
        assert_eq!(Agent::Codex.start(None, quote), "codex");
        assert_eq!(Agent::Claude.resume("0b6f7c2e"), "claude --resume 0b6f7c2e");
        assert_eq!(Agent::Codex.resume("019d7c54"), "codex resume 019d7c54");
        assert_eq!(Agent::of_program("-codex"), Some(Agent::Codex));
        assert_eq!(Agent::of_command("claude -n ABC-123"), Some(Agent::Claude));
        assert_eq!(Agent::of_command("aider"), None);
    }

    #[test]
    fn reads_each_login_check() {
        assert!(Agent::Claude.logged_in_from(r#"{"loggedIn": true, "authMethod": "claude.ai"}"#, true));
        assert!(!Agent::Claude.logged_in_from(r#"{"loggedIn": false}"#, true));
        assert!(Agent::Claude.logged_in_from("{\"loggedIn\": true}\nwarning: something\n", true));
        assert!(Agent::Codex.logged_in_from("Logged in using ChatGPT\n", true));
        assert!(!Agent::Codex.logged_in_from("Not logged in\n", false));
    }

    #[test]
    fn picks_the_last_ready_agent_else_the_first() {
        let found = |claude: bool, codex: bool| {
            vec![
                Found { agent: Agent::Claude, installed: claude, logged_in: claude },
                Found { agent: Agent::Codex, installed: codex, logged_in: codex },
            ]
        };
        assert_eq!(pick(&found(true, true), Some(Agent::Codex)), Agent::Codex);
        assert_eq!(pick(&found(true, false), Some(Agent::Codex)), Agent::Claude, "Codex isn't there");
        assert_eq!(pick(&found(false, true), None), Agent::Codex, "the only one");
        assert_eq!(pick(&found(false, false), None), Agent::Claude);
    }
}
