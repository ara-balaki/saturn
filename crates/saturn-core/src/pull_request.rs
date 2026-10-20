//! The pull request a pane works on, as GitHub's `gh` reports it, so a pane can show its number and
//! whether it's open, a draft, merged or closed. A pane's pull request is the one last linked in its
//! terminal (an agent printing the one it opened, or a link you pasted), else its branch's, else one
//! that names the pane's ticket. The `gh` calls live in the app.

use serde::Deserialize;

/// The fields Saturn asks `gh pr view` for.
pub const GH_FIELDS: &str = "number,state,isDraft,url,title";

/// The fields Saturn asks `gh pr list` for, to find a ticket's pull request.
pub const GH_LIST_FIELDS: &str = "number,state,isDraft,url,title,headRefName,body";

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
    #[serde(default)]
    head_ref_name: String,
    #[serde(default)]
    body: String,
}

impl Raw {
    /// A draft is open on GitHub, but it reads differently, so it gets its own state.
    fn into_pull_request(self) -> Option<PullRequest> {
        let state = match self.state.as_str() {
            "OPEN" if self.is_draft => PrState::Draft,
            "OPEN" => PrState::Open,
            "MERGED" => PrState::Merged,
            "CLOSED" => PrState::Closed,
            _ => return None,
        };
        Some(PullRequest { number: self.number, title: self.title, url: self.url, state })
    }
}

/// Reads `gh pr view --json number,state,isDraft,url,title`.
pub fn parse(json: &str) -> Option<PullRequest> {
    serde_json::from_str::<Raw>(json).ok()?.into_pull_request()
}

/// The pull request of each branch from `gh pr list --state all --json …`: an open one before a
/// merged or closed one, the newest first.
pub fn by_branch(json: &str) -> std::collections::HashMap<String, PullRequest> {
    let mut found: Vec<(String, PullRequest)> = serde_json::from_str::<Vec<Raw>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|raw| {
            let branch = raw.head_ref_name.clone();
            raw.into_pull_request().map(|pr| (branch, pr))
        })
        .collect();
    // Best last, so it's the one the map keeps.
    found.sort_by_key(|(_, pr)| (!matches!(pr.state, PrState::Merged | PrState::Closed), pr.number));
    found.into_iter().collect()
}

/// The pull request for a ticket from `gh pr list --search <ticket> --json …`: one whose title,
/// branch or description names it (GitHub's search also matches the ticket's parts on their own),
/// an open one before a merged or closed one, the newest first.
pub fn for_ticket(json: &str, ticket: &str) -> Option<PullRequest> {
    let found: Vec<Raw> = serde_json::from_str(json).ok()?;
    found
        .into_iter()
        .filter(|raw| [&raw.title, &raw.head_ref_name, &raw.body].iter().any(|text| names_ticket(text, ticket)))
        .filter_map(Raw::into_pull_request)
        .min_by_key(|pr| (matches!(pr.state, PrState::Merged | PrState::Closed), std::cmp::Reverse(pr.number)))
}

/// Whether `text` names `ticket` as a whole word, in any case: `ABC-12` is in "abc-12-fix" and
/// "fix: ABC-12", not in "ABC-123".
fn names_ticket(text: &str, ticket: &str) -> bool {
    let (text, ticket) = (text.to_ascii_lowercase(), ticket.to_ascii_lowercase());
    text.match_indices(&ticket).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + ticket.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric()) && !after.is_some_and(|c| c.is_ascii_alphanumeric())
    })
}

/// The ticket a pane's name is about: the first ticket-shaped word, `ABC-123`, upper-cased.
/// Claude's titles carry a mark before the name ("✳ ABC-123"), so every word is tried.
pub fn ticket_in(name: &str) -> Option<String> {
    name.split(|c: char| c.is_whitespace() || matches!(c, '/' | ':' | '(' | ')' | '[' | ']')).find_map(|word| {
        let (prefix, number) = word.split_once('-')?;
        let number = number.split('-').next().unwrap_or_default();
        let valid = prefix.len() >= 2
            && prefix.chars().all(|c| c.is_ascii_alphanumeric())
            && prefix.starts_with(|c: char| c.is_ascii_alphabetic())
            && !number.is_empty()
            && number.chars().all(|c| c.is_ascii_digit());
        valid.then(|| format!("{}-{number}", prefix.to_ascii_uppercase()))
    })
}

/// How much of a terminal's output is kept between reads, so a link split across two reads is
/// still found: longer than any pull request link (GitHub's owner and repository names are short).
const TAIL: usize = 256;

/// Finds GitHub pull request links in a terminal's output as it arrives, without reading the
/// screen again: an agent printing the pull request it opened, or a link you pasted.
#[derive(Debug, Default)]
pub struct LinkScanner {
    /// The end of the last read, which a link may continue from.
    tail: Vec<u8>,
}

impl LinkScanner {
    /// Reads more output and returns the last complete pull request link in it, as
    /// `https://github.com/owner/repo/pull/123`. A link cut off at the end of the read waits for
    /// the next one.
    pub fn feed(&mut self, bytes: &[u8]) -> Option<String> {
        const HOST: &[u8] = b"github.com/";
        let mut text = std::mem::take(&mut self.tail);
        text.extend_from_slice(bytes);
        let mut last = None;
        // Everything before `read` has been read; a link cut off at the end starts there.
        let mut read = 0;
        while let Some(at) = find(&text[read..], HOST).map(|at| read + at) {
            match link_at(&text, at + HOST.len()) {
                Link::Found(link, end) => {
                    last = Some(link);
                    read = end;
                }
                Link::Unfinished => {
                    read = at;
                    break;
                }
                Link::None => read = at + 1,
            }
        }
        // The unread end, which may hold the start of a link (even of `github.com/` itself).
        let keep = read.max(text.len().saturating_sub(TAIL));
        self.tail = text[keep..].to_vec();
        last
    }
}

enum Link {
    Found(String, usize),
    /// The output ends before the link does.
    Unfinished,
    None,
}

/// Reads `owner/repo/pull/123` from `at` on. While nothing after `at` has ended the link (a space,
/// a bracket, an escape), more of it may still come.
fn link_at(text: &[u8], at: usize) -> Link {
    let link_byte = |b: &u8| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/');
    let end = at + text[at..].iter().take_while(|b| link_byte(b)).count();
    if end == text.len() {
        return Link::Unfinished;
    }
    // A sentence may end right after the link: its full stop isn't part of the link.
    let end = end - text[at..end].iter().rev().take_while(|&&b| b == b'.').count();
    let mut parts = std::str::from_utf8(&text[at..end]).unwrap_or_default().split('/');
    let (Some(owner), Some(repo), Some("pull"), Some(number)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Link::None;
    };
    let valid = !owner.is_empty()
        && !repo.is_empty()
        && !number.is_empty()
        && owner.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && number.bytes().all(|b| b.is_ascii_digit());
    if !valid {
        return Link::None;
    }
    Link::Found(format!("https://github.com/{owner}/{repo}/pull/{number}"), end)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_each_branch_its_best_pull_request() {
        let json = r#"[
            {"number": 3, "state": "CLOSED", "url": "u3", "headRefName": "abc-1"},
            {"number": 7, "state": "OPEN", "isDraft": true, "url": "u7", "headRefName": "abc-1"},
            {"number": 5, "state": "MERGED", "url": "u5", "headRefName": "abc-2"}
        ]"#;
        let found = super::by_branch(json);
        assert_eq!(found["abc-1"].number, 7);
        assert_eq!(found["abc-2"].state, super::PrState::Merged);
    }

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
    fn finds_the_last_link_in_output() {
        let mut scanner = LinkScanner::default();
        let output = b"PR #158\r\n(https://github.com/acme/acme-web/pull/157), then \x1b[34mhttps://github.com/acme/acme-web/pull/158/files\x1b[0m.";
        assert_eq!(scanner.feed(output).as_deref(), Some("https://github.com/acme/acme-web/pull/158"));
        assert_eq!(scanner.feed(b"nothing new"), None, "a link is reported once");
        assert_eq!(
            scanner.feed(b"See https://github.com/acme/web.site/pull/7.\r\n").as_deref(),
            Some("https://github.com/acme/web.site/pull/7")
        );
    }

    #[test]
    fn finds_a_link_split_between_reads() {
        let mut scanner = LinkScanner::default();
        assert_eq!(scanner.feed(b"opened https://github.com/acme/acme-web/pu"), None);
        assert_eq!(scanner.feed(b"ll/42"), None, "the number may go on");
        assert_eq!(scanner.feed(b"1 for review").as_deref(), Some("https://github.com/acme/acme-web/pull/421"));
    }

    #[test]
    fn ignores_links_that_are_not_pull_requests() {
        let mut scanner = LinkScanner::default();
        let output =
            b"https://github.com/acme/acme-web/pulls https://github.com/acme/acme-web/issues/3 github.com/acme";
        assert_eq!(scanner.feed(output), None);
        assert_eq!(scanner.feed(b"/acme-web/pull/9 "), Some("https://github.com/acme/acme-web/pull/9".into()));
    }

    #[test]
    fn reads_a_ticket_from_a_pane_name() {
        assert_eq!(ticket_in("ABC-123").as_deref(), Some("ABC-123"));
        assert_eq!(ticket_in("✳ abc-123 review").as_deref(), Some("ABC-123"));
        assert_eq!(ticket_in("feat/abc-12-dark-mode").as_deref(), Some("ABC-12"));
        assert_eq!(ticket_in("zsh"), None);
        assert_eq!(ticket_in("fix/vat-rounding"), None);
    }

    #[test]
    fn picks_a_tickets_open_pull_request_first() {
        let list = r#"[
            {"number":9,"state":"MERGED","isDraft":false,"url":"u9","title":"ABC-12: first try","headRefName":"x","body":""},
            {"number":11,"state":"OPEN","isDraft":false,"url":"u11","title":"Dark mode","headRefName":"abc-12-dark","body":""},
            {"number":12,"state":"OPEN","isDraft":false,"url":"u12","title":"ABC-123 is another ticket","headRefName":"y","body":""}
        ]"#;
        assert_eq!(for_ticket(list, "ABC-12").map(|pr| pr.number), Some(11));
        assert_eq!(for_ticket(list, "ABC-99"), None);
        assert_eq!(for_ticket("[]", "ABC-12"), None);
    }

    #[test]
    fn anything_else_is_no_pull_request() {
        assert_eq!(parse("no pull requests found for branch \"main\""), None);
        assert_eq!(parse(&view("UNKNOWN", false)), None);
    }
}
