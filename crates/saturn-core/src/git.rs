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

/// What a review covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// Everything not committed yet: staged, unstaged and new files, against `HEAD`.
    #[default]
    Uncommitted,
    /// Everything the branch changed since it left the default branch, committed or not: what a
    /// pull request would show, plus work in progress.
    Branch,
}

/// The unified diff for `scope` in `repo`, new files included. Empty outside a repository.
pub fn patch(repo: &Path, scope: Scope) -> String {
    let base = match scope {
        Scope::Uncommitted => "HEAD".to_owned(),
        Scope::Branch => merge_base(repo).unwrap_or_else(|| "HEAD".to_owned()),
    };
    let mut patch = run(repo, &["diff", "--no-color", "--no-ext-diff", "-M", &base]).unwrap_or_default();
    // `git diff` leaves out files git does not track yet; show them as added in full.
    let untracked = run(repo, &["ls-files", "--others", "--exclude-standard", "-z"]).unwrap_or_default();
    for file in untracked.split('\0').filter(|file| !file.is_empty()) {
        // `--no-index` exits with 1 when the files differ, which they always do here.
        if let Ok(output) = git(repo).args(["diff", "--no-color", "--no-index", "--", "/dev/null", file]).output() {
            patch.push_str(&String::from_utf8_lossy(&output.stdout));
        }
    }
    patch
}

/// The checked-out branch, or `None` when detached or outside a repository.
pub fn branch(repo: &Path) -> Option<String> {
    run(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]).map(|name| name.trim().to_owned())
}

/// Where the current branch left the default branch (`origin/HEAD`, else `main` or `master`).
pub fn merge_base(repo: &Path) -> Option<String> {
    let default_branch = run(repo, &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"])
        .map(|name| name.trim().to_owned())
        .or_else(|| {
            ["main", "master"]
                .into_iter()
                .find(|name| run(repo, &["rev-parse", "--verify", "--quiet", name]).is_some())
                .map(str::to_owned)
        })?;
    run(repo, &["merge-base", "HEAD", &default_branch]).map(|sha| sha.trim().to_owned())
}

/// A worktree for `ticket` next to `repo` (`../<repo>-<ticket>`) on a branch named after the
/// ticket, made from the current `HEAD`, or the one already there. Returns its folder.
pub fn ticket_worktree(repo: &Path, ticket: &str) -> Result<PathBuf, String> {
    let branch: String =
        ticket.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
    let top = toplevel(repo).ok_or_else(|| format!("{} is not a git repository", repo.display()))?;
    let name = top.file_name().map_or_else(|| "repo".into(), |name| name.to_string_lossy().into_owned());
    let dir = top.parent().unwrap_or(&top).join(format!("{name}-{branch}"));
    if dir.join(".git").exists() {
        return Ok(dir);
    }
    let dir_arg = dir.to_string_lossy().into_owned();
    let branch_exists = run(&top, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_some();
    let args: Vec<&str> = if branch_exists {
        vec!["worktree", "add", &dir_arg, &branch]
    } else {
        vec!["worktree", "add", "-b", &branch, &dir_arg]
    };
    let output = git(&top).args(&args).output().map_err(|error| format!("git could not run: {error}"))?;
    if output.status.success() {
        Ok(dir)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).lines().next().unwrap_or("git worktree add failed").to_owned())
    }
}

fn git(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("--no-optional-locks").current_dir(repo);
    command
}

/// Runs git and returns its output when it succeeds.
fn run(repo: &Path, args: &[&str]) -> Option<String> {
    let output = git(repo).args(args).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
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

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temp dir");
        let sh = |args: &[&str]| {
            let status = Command::new("git").args(args).current_dir(dir.path()).output().expect("git runs").status;
            assert!(status.success(), "git {args:?}");
        };
        sh(&["init", "--quiet", "--initial-branch=main"]);
        sh(&["config", "user.email", "test@saturn.local"]);
        sh(&["config", "user.name", "Saturn tests"]);
        std::fs::write(dir.path().join("orders.ts"), "export const a = 1;\n").expect("write");
        sh(&["add", "."]);
        sh(&["commit", "--quiet", "-m", "first"]);
        sh(&["switch", "--quiet", "-c", "abc-236"]);
        std::fs::write(dir.path().join("filter.ts"), "export const f = 2;\n").expect("write");
        sh(&["add", "."]);
        sh(&["commit", "--quiet", "-m", "on the branch"]);
        std::fs::write(dir.path().join("orders.ts"), "export const a = 10;\n").expect("write");
        std::fs::write(dir.path().join(".env"), "PORT=4000\n").expect("write");
        dir
    }

    #[test]
    fn uncommitted_covers_edits_and_new_files() {
        let dir = repo();
        let patch = patch(dir.path(), Scope::Uncommitted);
        assert!(patch.contains("+export const a = 10;"), "{patch}");
        assert!(patch.contains("+PORT=4000"), "{patch}");
        assert!(!patch.contains("filter.ts"), "the branch's commit is not uncommitted: {patch}");
    }

    #[test]
    fn branch_covers_its_commits_and_work_in_progress() {
        let dir = repo();
        let patch = patch(dir.path(), Scope::Branch);
        assert!(patch.contains("+export const f = 2;"), "{patch}");
        assert!(patch.contains("+export const a = 10;"), "{patch}");
        assert!(patch.contains("+PORT=4000"), "{patch}");
    }

    #[test]
    fn the_branch_is_the_checked_out_one() {
        let dir = repo();
        assert_eq!(branch(dir.path()).as_deref(), Some("abc-236"));
        assert_eq!(
            toplevel(&dir.path().join(".")).map(|top| top.ends_with(dir.path().file_name().expect("a name"))),
            Some(true)
        );
    }

    #[test]
    fn a_ticket_gets_a_sibling_worktree_once() {
        let dir = repo();
        let first = ticket_worktree(dir.path(), "ABC-240").expect("created");
        assert!(first.ends_with(format!("{}-abc-240", dir.path().file_name().expect("a name").to_string_lossy())));
        assert_eq!(branch(&first).as_deref(), Some("abc-240"));
        assert_eq!(ticket_worktree(dir.path(), "ABC-240").expect("reused"), first);
        let _ = std::fs::remove_dir_all(&first);
    }

    #[test]
    fn a_folder_outside_git_has_no_status() {
        let dir = tempfile::tempdir().expect("a temp dir");
        assert!(status(dir.path()).is_empty());
    }
}
