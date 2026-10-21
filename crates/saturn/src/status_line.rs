//! Saturn's status line in Claude Code: printing it (`saturn status-line`, which Claude Code runs
//! after each message) and turning it on and off in Claude Code's own settings.

use std::io::Read as _;
use std::path::PathBuf;

use saturn_core::status_line;
use serde_json::Value;

/// The argument Claude Code runs Saturn with to get the status line.
pub const ARGUMENT: &str = "status-line";

/// Reads Claude Code's session state from stdin and prints the status line. Runs without a window.
pub fn print() {
    let mut text = String::new();
    let _ = std::io::stdin().read_to_string(&mut text);
    let input: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let steps = input
        .get("transcript_path")
        .and_then(Value::as_str)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|transcript| status_line::steps_in_transcript(&transcript));
    // This session's part of the plan's session, as Saturn last worked it out; outside Saturn there's
    // no such file and the line goes without.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs());
    let here = input.get("session_id").and_then(Value::as_str).and_then(|session| {
        let file = std::fs::read_to_string(shares_path()?).ok()?;
        status_line::share_of(&file, session, i64::try_from(now).unwrap_or(i64::MAX))
    });
    print!("{}", status_line::line(&input, steps.as_ref(), here.as_deref()));
}

/// Where Saturn keeps each session's share of the plan's session, for the status line.
pub fn shares_path() -> Option<PathBuf> {
    crate::settings::data_dir().map(|dir| dir.join("session-shares.json"))
}

/// Claude Code's user settings: `~/.claude/settings.json`, or under `CLAUDE_CONFIG_DIR`.
fn claude_settings_file() -> Option<PathBuf> {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")))?;
    Some(dir.join("settings.json"))
}

/// Where the status line Saturn replaced waits to be put back.
fn previous_file() -> Option<PathBuf> {
    crate::settings::data_dir().map(|dir| dir.join("previous-status-line.json"))
}

fn read_claude_settings() -> Value {
    claude_settings_file()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()))
}

/// Whether Claude Code shows Saturn's status line.
pub fn is_installed() -> bool {
    status_line::installed(&read_claude_settings())
}

/// Turns Saturn's status line on (keeping the one it replaces) or off (putting that one back).
pub fn set_installed(on: bool) -> Result<(), String> {
    let file = claude_settings_file().ok_or("Couldn't find Claude Code's settings.")?;
    let previous_file = previous_file().ok_or("Couldn't find Saturn's settings folder.")?;
    let mut settings = read_claude_settings();
    if on == status_line::installed(&settings) {
        return Ok(());
    }
    if on {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        let command = format!("'{}' {ARGUMENT}", exe.display().to_string().replace('\'', "'\\''"));
        let previous = status_line::install(&mut settings, &command);
        if let Some(dir) = previous_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&previous_file, previous.to_string()).map_err(|error| error.to_string())?;
    } else {
        let previous = std::fs::read_to_string(&previous_file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(Value::Null);
        status_line::uninstall(&mut settings, previous);
        let _ = std::fs::remove_file(&previous_file);
    }
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text = serde_json::to_string_pretty(&settings).map_err(|error| error.to_string())?;
    // Written whole to a neighbour first, so Claude Code never reads half a file.
    let partial = file.with_extension("json.saturn");
    std::fs::write(&partial, text + "\n").map_err(|error| error.to_string())?;
    std::fs::rename(&partial, &file).map_err(|error| error.to_string())
}
