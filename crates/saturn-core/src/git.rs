//! What git says about the files in a repository, for the tree's badges and the Changes list.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl FileState {
    /// The one-letter badge shown after the file's name.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Untracked => "?",
            Self::Conflicted => "!",
        }
    }
}

/// Changed files by absolute path, from `git status` in `repo`. Empty when `repo` is not a git
/// repository or git is missing.
pub fn status(repo: &Path) -> HashMap<PathBuf, FileState> {
    let Ok(output) = Command::new("git")
        // A read must not write: without this, `git status` refreshes .git/index, which the file
        // watcher sees, which runs `git status` again.
        .args(["--no-optional-locks", "status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .current_dir(repo)
        .output()
    else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }
    let Some(top) = toplevel(repo) else { return HashMap::new() };
    parse_porcelain(&output.stdout).into_iter().map(|(path, state)| (top.join(path), state)).collect()
}

/// The repository's top folder, which `git status` paths are relative to.
pub fn toplevel(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git").args(["rev-parse", "--show-toplevel"]).current_dir(dir).output().ok()?;
    output.status.success().then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

/// Parses `git status --porcelain=v1 -z`: entries of `XY path`, NUL-separated; a rename or copy
/// is followed by its old path as one more entry.
pub fn parse_porcelain(raw: &[u8]) -> Vec<(PathBuf, FileState)> {
    let mut entries = raw.split(|byte| *byte == 0).filter(|entry| entry.len() > 3);
    let mut files = Vec::new();
    while let Some(entry) = entries.next() {
        let (x, y) = (entry[0], entry[1]);
        let path = PathBuf::from(String::from_utf8_lossy(&entry[3..]).into_owned());
        let state = match (x, y) {
            (b'?', b'?') => FileState::Untracked,
            (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D') => FileState::Conflicted,
            (b'R' | b'C', _) => {
                entries.next(); // the old path
                FileState::Renamed
            }
            (b'A', _) => FileState::Added,
            (b'D', _) | (_, b'D') => FileState::Deleted,
            _ => FileState::Modified,
        };
        files.push((path, state));
    }
    files
}

/// Whether anything under `dir` has changed, for the quiet dot on a closed folder.
pub fn has_changes_under<S: std::hash::BuildHasher>(changes: &HashMap<PathBuf, FileState, S>, dir: &Path) -> bool {
    changes.keys().any(|path| path.starts_with(dir) && path != dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_kind_of_change() {
        let raw =
            b" M apps/api/orders.ts\0A  apps/api/filter.ts\0 D old.ts\0?? .env.local\0R  new.ts\0was.ts\0UU merge.ts\0";
        let files = parse_porcelain(raw);
        assert_eq!(
            files,
            [
                (PathBuf::from("apps/api/orders.ts"), FileState::Modified),
                (PathBuf::from("apps/api/filter.ts"), FileState::Added),
                (PathBuf::from("old.ts"), FileState::Deleted),
                (PathBuf::from(".env.local"), FileState::Untracked),
                (PathBuf::from("new.ts"), FileState::Renamed),
                (PathBuf::from("merge.ts"), FileState::Conflicted),
            ]
        );
    }

    #[test]
    fn a_folder_has_changes_when_a_file_beneath_it_does() {
        let changes = HashMap::from([(PathBuf::from("/repo/apps/api/orders.ts"), FileState::Modified)]);
        assert!(has_changes_under(&changes, Path::new("/repo/apps")));
        assert!(!has_changes_under(&changes, Path::new("/repo/packages")));
    }

    #[test]
    fn a_folder_outside_git_has_no_status() {
        let dir = tempfile::tempdir().expect("a temp dir");
        assert!(status(dir.path()).is_empty());
    }
}
