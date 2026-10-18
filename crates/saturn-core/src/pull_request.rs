//! The pull request for a pane's branch, as GitHub's `gh pr view --json` reports it, so a pane can
//! show its number and whether it's open, a draft, merged or closed. The `gh` call lives in the app.

use serde::Deserialize;

/// The fields Saturn asks `gh pr view` for.
pub const GH_FIELDS: &str = "number,state,isDraft,url,title";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Draft,
    Merged,
    Closed,
}

impl PrState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Draft => "Draft",
            Self::Merged => "Merged",
            Self::Closed => "Closed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: PrState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    number: u64,
    state: String,
    #[serde(default)]
    is_draft: bool,
    url: String,
    #[serde(default)]
    title: String,
}

/// Reads `gh pr view --json number,state,isDraft,url,title`. A draft is open on GitHub, but it
/// reads differently, so it gets its own state.
pub fn parse(json: &str) -> Option<PullRequest> {
    let raw: Raw = serde_json::from_str(json).ok()?;
    let state = match raw.state.as_str() {
        "OPEN" if raw.is_draft => PrState::Draft,
        "OPEN" => PrState::Open,
        "MERGED" => PrState::Merged,
        "CLOSED" => PrState::Closed,
        _ => return None,
    };
    Some(PullRequest { number: raw.number, title: raw.title, url: raw.url, state })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(state: &str, draft: bool) -> String {
        format!(
            r#"{{"isDraft":{draft},"number":42,"state":"{state}","title":"Add dark mode","url":"https://github.com/acme/web/pull/42"}}"#
        )
    }

    #[test]
    fn reads_each_state() {
        assert_eq!(parse(&view("OPEN", false)).map(|pr| pr.state), Some(PrState::Open));
        assert_eq!(parse(&view("OPEN", true)).map(|pr| pr.state), Some(PrState::Draft));
        assert_eq!(parse(&view("MERGED", false)).map(|pr| pr.state), Some(PrState::Merged));
        assert_eq!(parse(&view("CLOSED", false)).map(|pr| pr.state), Some(PrState::Closed));
    }

    #[test]
    fn keeps_the_number_title_and_link() {
        let pr = parse(&view("OPEN", false)).expect("parses");
        assert_eq!(pr.number, 42);
        assert_eq!(pr.title, "Add dark mode");
        assert_eq!(pr.url, "https://github.com/acme/web/pull/42");
    }

    #[test]
    fn anything_else_is_no_pull_request() {
        assert_eq!(parse("no pull requests found for branch \"main\""), None);
        assert_eq!(parse(&view("UNKNOWN", false)), None);
    }
}
