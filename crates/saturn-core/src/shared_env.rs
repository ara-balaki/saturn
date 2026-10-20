//! One set of env files for a repository and all its worktrees. Git ignores env files, so a new
//! worktree starts without them, and a copy drifts the moment a value is added on one side. Each
//! worktree gets links to the main checkout's files instead: there is one file per path, and a
//! value added in any worktree is in all of them at once.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What sharing did with one env file in a worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Now a link to the main checkout's file.
    Linked,
    /// A link already, left as it is.
    AlreadyShared,
    /// The worktree's own file, different from main's: never overwritten.
    OwnCopy,
}

/// Whether a file name is an env file: `.env`, `.env.local`, `production.env`. Templates meant
/// to be committed (`.env.example`) are not.
pub fn is_env_file(name: &str) -> bool {
    let name = name.to_lowercase();
    let template = [".example", ".sample", ".template", ".dist"].iter().any(|end| name.ends_with(end));
    !template
        && (name == ".env"
            || name.starts_with(".env.")
            || Path::new(&name).extension().is_some_and(|extension| extension == "env"))
}

/// The env files of a checkout that git ignores, relative to it. Ignored folders
/// (`node_modules`) are skipped whole.
pub fn env_files(checkout: &Path) -> Vec<PathBuf> {
    let Ok(output) = Command::new("git")
        .args(["--no-optional-locks", "ls-files", "-z", "--others", "--ignored", "--exclude-standard", "--directory"])
        .current_dir(checkout)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|entry| !entry.is_empty() && !entry.ends_with('/'))
        .map(PathBuf::from)
        .filter(|path| path.file_name().is_some_and(|name| is_env_file(&name.to_string_lossy())))
        .collect()
}

/// Links each of the main checkout's env files into `worktree`, where the worktree has none or the
/// same contents. Nothing happens in the main checkout itself, or outside a repository.
pub fn share(worktree: &Path) -> Vec<(PathBuf, Outcome)> {
    let Some(main) = crate::git::main_worktree(worktree) else { return Vec::new() };
    let (Ok(main), Ok(worktree)) = (main.canonicalize(), worktree.canonicalize()) else { return Vec::new() };
    if main == worktree {
        return Vec::new();
    }
    env_files(&main)
        .into_iter()
        .filter_map(|relative| {
            let outcome = link(&main.join(&relative), &worktree.join(&relative)).ok()??;
            Some((relative, outcome))
        })
        .collect()
}

/// Makes `target` a link to `source`. `None` when the folder it would go in doesn't exist in this
/// worktree (another branch, without that app).
fn link(source: &Path, target: &Path) -> io::Result<Option<Outcome>> {
    match fs::symlink_metadata(target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if !target.parent().is_some_and(Path::is_dir) {
                return Ok(None);
            }
            std::os::unix::fs::symlink(source, target)?;
            Ok(Some(Outcome::Linked))
        }
        Err(error) => Err(error),
        Ok(meta) if meta.file_type().is_symlink() => Ok(Some(Outcome::AlreadyShared)),
        Ok(meta) if meta.is_file() && fs::read(target)? == fs::read(source)? => {
            // A copy made by hand: swapped for the link in one step, so it's never missing.
            let temporary = target.with_file_name(format!(
                ".{}.saturn-link",
                target.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned())
            ));
            let _ = fs::remove_file(&temporary);
            std::os::unix::fs::symlink(source, &temporary)?;
            fs::rename(&temporary, target)?;
            Ok(Some(Outcome::Linked))
        }
        Ok(_) => Ok(Some(Outcome::OwnCopy)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(dir).output().expect("git runs").status;
        assert!(status.success(), "git {args:?}");
    }

    /// A repository with a root `.env`, an app's `.env.local` and an ignored `node_modules`, and a
    /// worktree beside it. Returns (main, worktree).
    fn repo_with_worktree(dir: &Path) -> (PathBuf, PathBuf) {
        let main = dir.join("acme-web");
        fs::create_dir_all(main.join("apps/api")).expect("make folders");
        fs::create_dir_all(main.join("node_modules/pkg")).expect("make folders");
        git(&main, &["init", "-q", "-b", "main"]);
        git(&main, &["config", "user.email", "test@example.com"]);
        git(&main, &["config", "user.name", "Test"]);
        fs::write(main.join(".gitignore"), ".env\n.env.local\nnode_modules\n").expect("write");
        fs::write(main.join(".env.example"), "PORT=\n").expect("write");
        fs::write(main.join("apps/api/index.ts"), "export {}\n").expect("write");
        git(&main, &["add", "."]);
        git(&main, &["commit", "-q", "-m", "start"]);
        fs::write(main.join(".env"), "PORT=4000\n").expect("write");
        fs::write(main.join("apps/api/.env.local"), "SECRET=1\n").expect("write");
        fs::write(main.join("node_modules/pkg/.env"), "NOT=ours\n").expect("write");
        let worktree = dir.join("acme-web-abc-123");
        git(&main, &["worktree", "add", "-q", "-b", "abc-123", &worktree.to_string_lossy()]);
        (main, worktree)
    }

    #[test]
    fn knows_env_files_from_templates() {
        assert!(is_env_file(".env") && is_env_file(".env.local") && is_env_file("production.env"));
        assert!(!is_env_file(".env.example") && !is_env_file("env.ts") && !is_env_file(".envrc"));
    }

    #[test]
    fn finds_ignored_env_files_but_not_inside_ignored_folders() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let (main, _) = repo_with_worktree(dir.path());
        let mut files = env_files(&main);
        files.sort();
        assert_eq!(files, [PathBuf::from(".env"), PathBuf::from("apps/api/.env.local")]);
    }

    #[test]
    fn a_worktree_shares_main_env_files_both_ways() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let (main, worktree) = repo_with_worktree(dir.path());
        let outcomes = share(&worktree);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|(_, outcome)| *outcome == Outcome::Linked));

        // Added in main, there in the worktree; added in the worktree, there in main.
        fs::write(main.join(".env"), "PORT=4000\nSENTRY_DSN=a\n").expect("write");
        assert!(fs::read_to_string(worktree.join(".env")).expect("read").contains("SENTRY_DSN"));
        fs::write(worktree.join("apps/api/.env.local"), "SECRET=2\n").expect("write");
        assert_eq!(fs::read_to_string(main.join("apps/api/.env.local")).expect("read"), "SECRET=2\n");

        assert!(share(&worktree).iter().all(|(_, outcome)| *outcome == Outcome::AlreadyShared));
        assert!(share(&main).is_empty(), "main is left alone");
    }

    #[test]
    fn a_worktree_keeps_its_own_different_file() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let (main, worktree) = repo_with_worktree(dir.path());
        fs::write(worktree.join(".env"), "PORT=5000\n").expect("write");
        fs::copy(main.join("apps/api/.env.local"), worktree.join("apps/api/.env.local")).expect("copy");
        let mut outcomes = share(&worktree);
        outcomes.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            outcomes,
            [(PathBuf::from(".env"), Outcome::OwnCopy), (PathBuf::from("apps/api/.env.local"), Outcome::Linked)]
        );
        assert_eq!(fs::read_to_string(worktree.join(".env")).expect("read"), "PORT=5000\n");
        assert!(fs::symlink_metadata(worktree.join("apps/api/.env.local")).expect("exists").file_type().is_symlink());
    }
}
