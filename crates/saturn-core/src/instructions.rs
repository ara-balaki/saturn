//! What an agent in a folder is told, found the way Claude Code finds it: instruction files from the
//! folder up to the repository (`CLAUDE.md`, `AGENTS.md`, `CLAUDE.local.md`) and the personal one,
//! rules (`.claude/rules/`, some only for certain files), skills, commands, subagents, hooks and MCP
//! servers. And what's said twice: the same instruction in several files drifts apart the first time
//! one is edited, and a model reading both can't tell which one wins.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Instructions,
    Rule,
    Skill,
    Command,
    Subagent,
    Hook,
    McpServer,
}

impl Kind {
    pub fn heading(self) -> &'static str {
        match self {
            Self::Instructions => "Instructions",
            Self::Rule => "Rules",
            Self::Skill => "Skills",
            Self::Command => "Commands",
            Self::Subagent => "Subagents",
            Self::Hook => "Hooks",
            Self::McpServer => "MCP servers",
        }
    }
}

/// One thing the agent is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: Kind,
    /// What it's called: the file's place, a skill's name, a hook's event.
    pub name: String,
    /// What it says it's for, or which files it's for.
    pub detail: Option<String>,
    /// Where it is, to open.
    pub path: PathBuf,
    /// Its length in lines, for files.
    pub lines: usize,
}

/// Instruction files longer than this get a word: models follow short ones better (Anthropic's
/// advice: "ruthlessly prune").
pub const LONG: usize = 200;

/// `name:`, `description:` and `paths:` from a Markdown file's front matter (between `---` lines).
fn front_matter(text: &str) -> HashMap<String, String> {
    let mut found = HashMap::new();
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return found;
    }
    let mut key: Option<String> = None;
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some(item) = line.trim().strip_prefix("- ").filter(|_| line.starts_with(' ') || line.starts_with('-')) {
            if let Some(key) = &key {
                let entry: &mut String = found.entry(key.clone()).or_default();
                if !entry.is_empty() {
                    entry.push_str(", ");
                }
                entry.push_str(item.trim().trim_matches(['"', '\'']));
            }
        } else if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_owned();
            let value = value.trim().trim_matches(['"', '\'']).to_owned();
            key = Some(name.clone());
            if !value.is_empty() {
                found.insert(name, value);
            }
        }
    }
    found
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Markdown files in a folder, sorted, and those one level down named `file` (skills' `SKILL.md`).
fn markdown_in(folder: &Path, nested: Option<&str>) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| match nested {
            Some(file) => Some(path.join(file)).filter(|inner| inner.is_file()),
            None => (path.extension().is_some_and(|extension| extension == "md")).then_some(path),
        })
        .collect();
    found.sort();
    found
}

/// Everything an agent working in `folder` (inside `checkout`) is given; `home` is Claude Code's own
/// folder (`~/.claude`), for the personal instructions, skills and commands.
pub fn gather(checkout: &Path, folder: &Path, home: Option<&Path>) -> Vec<Source> {
    let mut found = Vec::new();
    let place = |path: &Path| {
        path.strip_prefix(checkout)
            .map_or_else(|_| path.display().to_string(), |relative| relative.display().to_string())
    };
    let mut add_file = |kind: Kind, path: PathBuf, name: String, detail: Option<String>| {
        if let Some(text) = read(&path) {
            found.push(Source { kind, name, detail, path, lines: text.lines().count() });
        }
    };
    // Instruction files: the personal one, then from the repository's top down to the folder.
    if let Some(home) = home {
        add_file(
            Kind::Instructions,
            home.join("CLAUDE.md"),
            "Your own CLAUDE.md".to_owned(),
            Some("for every project".to_owned()),
        );
    }
    let mut folders: Vec<PathBuf> =
        folder.ancestors().take_while(|path| path.starts_with(checkout)).map(Path::to_path_buf).collect();
    folders.reverse();
    for dir in &folders {
        for file in ["CLAUDE.md", "AGENTS.md", "CLAUDE.local.md", ".claude/CLAUDE.md"] {
            let path = dir.join(file);
            let name = place(&path);
            add_file(Kind::Instructions, path, name, None);
        }
    }
    let claude = checkout.join(".claude");
    for path in markdown_in(&claude.join("rules"), None) {
        let text = read(&path).unwrap_or_default();
        let paths = front_matter(&text).remove("paths").map(|paths| format!("for {paths}"));
        add_file(Kind::Rule, path.clone(), place(&path), paths.or_else(|| Some("for every file".to_owned())));
    }
    let mut named = |kind: Kind, folder: &Path, nested: Option<&str>, own: bool| {
        for path in markdown_in(folder, nested) {
            let text = read(&path).unwrap_or_default();
            let mut matter = front_matter(&text);
            let fallback = if nested.is_some() { path.parent() } else { Some(path.as_path()) }
                .and_then(Path::file_stem)
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut name = matter.remove("name").unwrap_or(fallback);
            if kind == Kind::Command {
                name = format!("/{name}");
            }
            if own {
                name.push_str(" (yours)");
            }
            add_file(kind, path, name, matter.remove("description"));
        }
    };
    named(Kind::Skill, &claude.join("skills"), Some("SKILL.md"), false);
    named(Kind::Command, &claude.join("commands"), None, false);
    named(Kind::Subagent, &claude.join("agents"), None, false);
    if let Some(home) = home {
        named(Kind::Skill, &home.join("skills"), Some("SKILL.md"), true);
        named(Kind::Command, &home.join("commands"), None, true);
        named(Kind::Subagent, &home.join("agents"), None, true);
    }
    found.extend(hooks(&claude.join("settings.json")));
    found.extend(hooks(&claude.join("settings.local.json")));
    found.extend(mcp_servers(&checkout.join(".mcp.json")));
    found
}

/// The hooks a settings file sets: each event, and what runs.
fn hooks(settings: &Path) -> Vec<Source> {
    let Some(json) = read(settings).and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()) else {
        return Vec::new();
    };
    let Some(events) = json.get("hooks").and_then(serde_json::Value::as_object) else { return Vec::new() };
    events
        .iter()
        .map(|(event, groups)| {
            let commands: Vec<String> = groups
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|group| group.get("hooks").and_then(serde_json::Value::as_array).cloned().unwrap_or_default())
                .filter_map(|hook| hook.get("command").and_then(serde_json::Value::as_str).map(str::to_owned))
                .collect();
            Source {
                kind: Kind::Hook,
                name: event.clone(),
                detail: Some(commands.join(" · ")),
                path: settings.to_owned(),
                lines: 0,
            }
        })
        .collect()
}

/// The MCP servers a project's `.mcp.json` names.
fn mcp_servers(file: &Path) -> Vec<Source> {
    let Some(json) = read(file).and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()) else {
        return Vec::new();
    };
    let Some(servers) = json.get("mcpServers").and_then(serde_json::Value::as_object) else { return Vec::new() };
    servers
        .iter()
        .map(|(name, server)| {
            let what = server
                .get("command")
                .or_else(|| server.get("url"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            Source { kind: Kind::McpServer, name: name.clone(), detail: what, path: file.to_owned(), lines: 0 }
        })
        .collect()
}

/// An instruction said in more than one place: the words, and where (file, line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repeat {
    pub text: String,
    pub places: Vec<(PathBuf, usize)>,
}

/// A line as it's compared: words only, lower-case, so `- **Never** push to main.` and
/// `Never push to main` are the same.
fn words_of(line: &str) -> String {
    line.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Instructions said in more than one of `sources`' files (instructions, rules and skills): lines of
/// six words or more that read the same once punctuation and case are set aside. A whole file that
/// copies another shows as one entry, line 1 in each.
pub fn repeats(sources: &[Source]) -> Vec<Repeat> {
    let files: Vec<&Source> =
        sources.iter().filter(|source| matches!(source.kind, Kind::Instructions | Kind::Rule | Kind::Skill)).collect();
    let texts: Vec<(PathBuf, String)> =
        files.iter().filter_map(|source| Some((source.path.clone(), read(&source.path)?))).collect();
    let mut found: Vec<Repeat> = Vec::new();
    // Whole copies first, so their lines aren't listed one by one.
    let mut copied: Vec<PathBuf> = Vec::new();
    for (index, (path, text)) in texts.iter().enumerate() {
        for (other, other_text) in &texts[index + 1..] {
            if !text.trim().is_empty() && words_of(text) == words_of(other_text) {
                found.push(Repeat {
                    text: "The whole file".to_owned(),
                    places: vec![(path.clone(), 1), (other.clone(), 1)],
                });
                copied.push(other.clone());
            }
        }
    }
    let mut lines: BTreeMap<String, (String, Vec<(PathBuf, usize)>)> = BTreeMap::new();
    for (path, text) in texts.iter().filter(|(path, _)| !copied.contains(path)) {
        let mut in_code = false;
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                in_code = !in_code;
                continue;
            }
            let key = words_of(line);
            if in_code || key.split(' ').count() < 6 || line.trim_start().starts_with('#') {
                continue;
            }
            let entry = lines
                .entry(key)
                .or_insert_with(|| (line.trim().trim_start_matches(['-', '*', ' ']).to_owned(), Vec::new()));
            if !entry.1.iter().any(|(seen, _)| seen == path) {
                entry.1.push((path.clone(), number + 1));
            }
        }
    }
    found.extend(
        lines.into_values().filter(|(_, places)| places.len() > 1).map(|(text, places)| Repeat { text, places }),
    );
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("a folder")).expect("make folders");
        fs::write(path, text).expect("write");
    }

    #[test]
    fn gathers_what_an_agent_is_told() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let (repo, home) = (dir.path().join("acme-web"), dir.path().join("home"));
        write(&repo, "CLAUDE.md", "# Acme\nRun the tests.\n");
        write(&repo, "apps/web/AGENTS.md", "Use the design tokens.\n");
        write(&repo, ".claude/rules/api.md", "---\npaths:\n  - \"apps/api/**\"\n---\nValidate input.\n");
        write(
            &repo,
            ".claude/skills/release/SKILL.md",
            "---\nname: release\ndescription: Ship a version\n---\nSteps.\n",
        );
        write(&repo, ".claude/commands/start.md", "Start a ticket.\n");
        write(
            &repo,
            ".claude/settings.json",
            r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"lint.sh"}]}]}}"#,
        );
        write(&repo, ".mcp.json", r#"{"mcpServers":{"db":{"command":"db-mcp"}}}"#);
        write(&home, "CLAUDE.md", "Be brief.\n");
        let found = gather(&repo, &repo.join("apps/web"), Some(&home));
        let named = |kind: Kind| {
            found.iter().filter(|source| source.kind == kind).map(|source| source.name.as_str()).collect::<Vec<_>>()
        };
        assert_eq!(named(Kind::Instructions), ["Your own CLAUDE.md", "CLAUDE.md", "apps/web/AGENTS.md"]);
        assert_eq!(
            found.iter().find(|source| source.kind == Kind::Rule).and_then(|rule| rule.detail.clone()).as_deref(),
            Some("for apps/api/**")
        );
        assert_eq!(named(Kind::Skill), ["release"]);
        assert_eq!(named(Kind::Command), ["/start"]);
        assert_eq!(
            found.iter().find(|source| source.kind == Kind::Hook).and_then(|hook| hook.detail.clone()).as_deref(),
            Some("lint.sh")
        );
        assert_eq!(named(Kind::McpServer), ["db"]);
    }

    #[test]
    fn finds_what_is_said_twice() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let repo = dir.path();
        write(
            repo,
            "CLAUDE.md",
            "# Rules\n- **Never** push to main without a review.\n- Short line here.\n```\nnever push to main without a review\n```\n",
        );
        write(repo, ".claude/rules/git.md", "Never push to main, without a review!\n");
        write(repo, "AGENTS.md", "Same file.\nWith two lines of six words or more.\n");
        write(repo, "apps/CLAUDE.md", "Same file.\nWith two lines of six words or more.\n");
        let mut sources = gather(repo, &repo.join("apps"), None);
        sources.retain(|source| source.kind != Kind::Hook);
        let found = repeats(&sources);
        assert!(found.iter().any(|repeat| repeat.text == "The whole file" && repeat.places.len() == 2));
        let push = found.iter().find(|repeat| repeat.text.contains("push to main")).expect("the push rule");
        assert_eq!(push.places.len(), 2, "the code block doesn't count: {push:?}");
        assert!(!found.iter().any(|repeat| repeat.text.contains("Short line")));
    }
}
