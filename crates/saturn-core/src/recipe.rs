//! A repository's worktree recipe, `.saturn/worktree.json`: how to make a fresh worktree ready
//! (`setup`: install, build, generate), how to start the app (`run`: named commands, one terminal
//! each), and how to tidy up before a worktree goes (`cleanup`). A fresh worktree has the code but
//! nothing git ignores, so it can't run until someone remembers the steps; the recipe remembers.
//!
//! ```json
//! {
//!   "setup": ["pnpm install", "pnpm --filter @acme/shared build"],
//!   "run": { "api": "pnpm --filter api dev", "web": "pnpm --filter web dev" },
//!   "cleanup": "docker compose down"
//! }
//! ```
//!
//! `setup` and `cleanup` are a command or a list run one after another, stopping at the first that
//! fails. Every field is optional.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Where the recipe lives in a checkout.
pub const FILE: &str = ".saturn/worktree.json";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recipe {
    pub setup: Vec<String>,
    /// Name and command, in the file's order.
    pub run: Vec<(String, String)>,
    pub cleanup: Vec<String>,
}

/// One command or several.
#[derive(Deserialize)]
#[serde(untagged)]
enum Steps {
    One(String),
    Many(Vec<String>),
}

impl Steps {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(step) => vec![step],
            Self::Many(steps) => steps,
        }
        .into_iter()
        .map(|step| step.trim().to_owned())
        .filter(|step| !step.is_empty())
        .collect()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    setup: Option<Steps>,
    run: Option<serde_json::Map<String, serde_json::Value>>,
    cleanup: Option<Steps>,
}

impl Recipe {
    /// Reads a recipe, saying what is wrong with it in words.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: File = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let run = file
            .run
            .unwrap_or_default()
            .into_iter()
            .map(|(name, command)| match command {
                serde_json::Value::String(command) if !command.trim().is_empty() => {
                    Ok((name, command.trim().to_owned()))
                }
                _ => Err(format!("\"run\": \"{name}\" needs a command, as text")),
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            setup: file.setup.map(Steps::into_vec).unwrap_or_default(),
            run,
            cleanup: file.cleanup.map(Steps::into_vec).unwrap_or_default(),
        })
    }

    /// The setup steps as one shell command, each running only if the one before succeeded.
    pub fn setup_command(&self) -> Option<String> {
        and_then(&self.setup)
    }

    /// The cleanup steps as one shell command.
    pub fn cleanup_command(&self) -> Option<String> {
        and_then(&self.cleanup)
    }
}

fn and_then(steps: &[String]) -> Option<String> {
    (!steps.is_empty()).then(|| {
        if steps.len() == 1 {
            steps[0].clone()
        } else {
            steps.iter().map(|step| format!("({step})")).collect::<Vec<_>>().join(" && ")
        }
    })
}

/// The recipe for `checkout`: its own file, else the main checkout's (a branch made before the
/// recipe existed still gets set up). `None` when neither has one; an error names the file.
pub fn load(checkout: &Path) -> Option<Result<Recipe, String>> {
    let own = checkout.join(FILE);
    let path: PathBuf = if own.is_file() {
        own
    } else {
        crate::git::main_worktree(checkout).map(|main| main.join(FILE)).filter(|path| path.is_file())?
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => return Some(Err(format!("{}: {error}", path.display()))),
    };
    Some(Recipe::parse(&text).map_err(|error| format!("{}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_steps_as_one_command_or_a_list() {
        let recipe = Recipe::parse(
            r#"{ "setup": ["pnpm install", " ", "pnpm build"], "run": { "api": "pnpm --filter api dev", "web": "vite" }, "cleanup": "docker compose down" }"#,
        )
        .expect("a valid recipe");
        assert_eq!(recipe.setup_command().as_deref(), Some("(pnpm install) && (pnpm build)"));
        assert_eq!(recipe.cleanup_command().as_deref(), Some("docker compose down"));
        assert_eq!(
            recipe.run,
            [("api".to_owned(), "pnpm --filter api dev".to_owned()), ("web".to_owned(), "vite".to_owned())]
        );
    }

    #[test]
    fn every_field_is_optional() {
        let recipe = Recipe::parse("{}").expect("an empty recipe is valid");
        assert_eq!(recipe, Recipe::default());
        assert_eq!(recipe.setup_command(), None);
    }

    #[test]
    fn says_what_is_wrong() {
        assert!(Recipe::parse(r#"{ "setpu": "pnpm install" }"#).expect_err("a typo").contains("setpu"));
        assert!(Recipe::parse(r#"{ "run": { "web": 5173 } }"#).expect_err("not a command").contains("web"));
        assert!(Recipe::parse("{ setup: ").is_err());
    }

    #[test]
    fn a_worktree_falls_back_to_the_main_checkout_recipe() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let main = dir.path().join("acme-web");
        std::fs::create_dir_all(main.join(".saturn")).expect("make folders");
        let git = |args: &[&str]| {
            let status = crate::git::command(&main).args(args).output().expect("git runs").status;
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&[
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=Test",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "start",
        ]);
        let worktree = dir.path().join("acme-web-abc-123");
        git(&["worktree", "add", "-q", "-b", "abc-123", &worktree.to_string_lossy()]);

        assert!(load(&worktree).is_none(), "no recipe anywhere");
        std::fs::write(main.join(FILE), r#"{ "setup": "pnpm install" }"#).expect("write");
        let recipe = load(&worktree).expect("main's recipe").expect("valid");
        assert_eq!(recipe.setup_command().as_deref(), Some("pnpm install"));

        std::fs::create_dir_all(worktree.join(".saturn")).expect("make folders");
        std::fs::write(worktree.join(FILE), "{ broken").expect("write");
        assert!(load(&worktree).expect("its own file").expect_err("broken").contains("worktree.json"));
    }
}
