//! Pull requests from GitHub through the `gh` CLI, already signed in on most machines that use
//! GitHub. Without `gh`, or outside a GitHub repository, there is simply no pull request.

use std::path::Path;
use std::process::{Command, Output};

use saturn_core::pull_request::{self, GH_FIELDS, GH_LIST_FIELDS, PullRequest};

/// A pull request by its branch, number or link, as `gh pr view` takes it, run in the repository
/// at `root` (a link works from any repository).
pub fn pull_request(root: &Path, which: &str) -> Option<PullRequest> {
    let output = gh(root, &["pr", "view", which, "--json", GH_FIELDS])?;
    pull_request::parse(&String::from_utf8_lossy(&output.stdout))
}

/// Every branch's pull request in the repository at `root` (the newest 200), in one call.
pub fn pull_requests_by_branch(root: &Path) -> std::collections::HashMap<String, PullRequest> {
    gh(root, &["pr", "list", "--state", "all", "--limit", "200", "--json", GH_LIST_FIELDS])
        .map(|output| pull_request::by_branch(&String::from_utf8_lossy(&output.stdout)))
        .unwrap_or_default()
}

/// The pull request about a ticket in the repository at `root`: one that names it, open first.
pub fn pull_request_for_ticket(root: &Path, ticket: &str) -> Option<PullRequest> {
    let output =
        gh(root, &["pr", "list", "--state", "all", "--limit", "20", "--search", ticket, "--json", GH_LIST_FIELDS])?;
    pull_request::for_ticket(&String::from_utf8_lossy(&output.stdout), ticket)
}

fn gh(root: &Path, args: &[&str]) -> Option<Output> {
    let mut command = Command::new("gh");
    command.args(args).current_dir(root);
    // Started from Finder, Saturn's own PATH lacks Homebrew's bin, where `gh` usually lives.
    let path =
        crate::LOGIN_PATH.get().cloned().unwrap_or_else(|| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_owned());
    command.env("PATH", path).env("GH_PROMPT_DISABLED", "1");
    let output = command.output().ok()?;
    output.status.success().then_some(output)
}
