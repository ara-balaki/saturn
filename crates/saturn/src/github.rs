//! Pull requests from GitHub through the `gh` CLI, already signed in on most machines that use
//! GitHub. Without `gh`, or outside a GitHub repository, there is simply no pull request.

use std::path::Path;
use std::process::Command;

use saturn_core::pull_request::{self, GH_FIELDS, PullRequest};

/// The pull request whose head is `branch` in the repository at `root`, if there is one.
pub fn pull_request(root: &Path, branch: &str) -> Option<PullRequest> {
    let mut command = Command::new("gh");
    command.args(["pr", "view", branch, "--json", GH_FIELDS]).current_dir(root);
    // Started from Finder, Saturn's own PATH lacks Homebrew's bin, where `gh` usually lives.
    let path =
        crate::LOGIN_PATH.get().cloned().unwrap_or_else(|| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_owned());
    command.env("PATH", path).env("GH_PROMPT_DISABLED", "1");
    let output = command.output().ok()?;
    output.status.success().then(|| pull_request::parse(&String::from_utf8_lossy(&output.stdout))).flatten()
}
