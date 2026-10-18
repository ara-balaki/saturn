//! ClickUp's tickets as Saturn needs them: an id to start an agent on, a title, and a status to
//! show on the pane. Parsed from ClickUp's API v2 (<https://developer.clickup.com>); the HTTP calls
//! live in the app.

use serde::Deserialize;

/// One task, as the picker and the pane header show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    /// ClickUp's own id (`86c1abc2`), for API calls and links.
    pub id: String,
    /// The workspace's custom id (`ABC-236`) when it uses them; what people call the ticket.
    pub custom_id: Option<String>,
    pub title: String,
    pub status: String,
    /// The status's colour in ClickUp, `#rrggbb`.
    pub status_color: Option<String>,
    pub url: String,
    pub list: Option<String>,
}

impl Ticket {
    /// What the ticket is called: its custom id, else ClickUp's id.
    pub fn key(&self) -> &str {
        self.custom_id.as_deref().unwrap_or(&self.id)
    }

    /// Whether a search matches its key, title or status, ignoring case.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || [self.key(), &self.title, &self.status].iter().any(|text| text.to_lowercase().contains(&query))
    }
}

#[derive(Deserialize)]
struct TaskJson {
    id: String,
    #[serde(default)]
    custom_id: Option<String>,
    name: String,
    status: StatusJson,
    url: String,
    #[serde(default)]
    list: Option<NamedJson>,
}

#[derive(Deserialize)]
struct StatusJson {
    status: String,
    #[serde(default)]
    color: Option<String>,
}

#[derive(Deserialize)]
struct NamedJson {
    name: String,
}

impl From<TaskJson> for Ticket {
    fn from(task: TaskJson) -> Self {
        Self {
            id: task.id,
            custom_id: task.custom_id.filter(|id| !id.is_empty()),
            title: task.name,
            status: task.status.status,
            status_color: task.status.color.filter(|color| color.starts_with('#')),
            url: task.url,
            list: task.list.map(|list| list.name),
        }
    }
}

/// How many tasks ClickUp returns per page.
const PAGE_SIZE: usize = 100;

/// A page of `GET /team/{team_id}/task`: its tickets, and whether more pages follow.
pub fn parse_task_page(json: &str) -> Result<(Vec<Ticket>, bool), serde_json::Error> {
    #[derive(Deserialize)]
    struct Page {
        tasks: Vec<TaskJson>,
        #[serde(default)]
        last_page: Option<bool>,
    }
    let page: Page = serde_json::from_str(json)?;
    // ClickUp says whether this was the last page; without that, a full page (100) may have more.
    let more = page.last_page.map_or(page.tasks.len() >= PAGE_SIZE, |last| !last);
    Ok((page.tasks.into_iter().map(Ticket::from).collect(), more))
}

/// One task from `GET /task/{id}`.
pub fn parse_task(json: &str) -> Result<Ticket, serde_json::Error> {
    serde_json::from_str::<TaskJson>(json).map(Ticket::from)
}

/// The signed-in user's id, from `GET /user`.
pub fn parse_user_id(json: &str) -> Result<u64, serde_json::Error> {
    #[derive(Deserialize)]
    struct Body {
        user: User,
    }
    #[derive(Deserialize)]
    struct User {
        id: u64,
    }
    serde_json::from_str::<Body>(json).map(|body| body.user.id)
}

/// The workspaces the token can see, from `GET /team`, as (id, name).
pub fn parse_workspaces(json: &str) -> Result<Vec<(String, String)>, serde_json::Error> {
    #[derive(Deserialize)]
    struct Body {
        teams: Vec<Team>,
    }
    #[derive(Deserialize)]
    struct Team {
        id: String,
        name: String,
    }
    serde_json::from_str::<Body>(json).map(|body| body.teams.into_iter().map(|team| (team.id, team.name)).collect())
}

/// Groups tickets by status for the picker, keeping ClickUp's order of first appearance.
pub fn by_status(tickets: &[Ticket]) -> Vec<(&str, Vec<&Ticket>)> {
    let mut groups: Vec<(&str, Vec<&Ticket>)> = Vec::new();
    for ticket in tickets {
        match groups.iter_mut().find(|(status, _)| *status == ticket.status) {
            Some((_, group)) => group.push(ticket),
            None => groups.push((&ticket.status, vec![ticket])),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r##"{
        "tasks": [
            {"id": "86c1abc2", "custom_id": "ABC-236", "name": "Dark mode toggle",
             "status": {"status": "building", "color": "#7b68ee", "type": "custom"},
             "url": "https://app.clickup.com/t/86c1abc2", "list": {"id": "9", "name": "Inbox"},
             "assignees": [{"id": 1}]},
            {"id": "86c1abc3", "custom_id": null, "name": "Filter on every list page",
             "status": {"status": "ready", "color": "#d3d3d3"},
             "url": "https://app.clickup.com/t/86c1abc3"}
        ],
        "last_page": false
    }"##;

    #[test]
    fn a_page_of_tasks_becomes_tickets() {
        let (tickets, more) = parse_task_page(PAGE).expect("parses");
        assert!(more);
        assert_eq!(tickets[0].key(), "ABC-236");
        assert_eq!(tickets[0].status, "building");
        assert_eq!(tickets[0].list.as_deref(), Some("Inbox"));
        assert_eq!(tickets[1].key(), "86c1abc3", "without a custom id, ClickUp's id names it");
    }

    #[test]
    fn search_looks_at_the_key_the_title_and_the_status() {
        let (tickets, _) = parse_task_page(PAGE).expect("parses");
        assert!(tickets[0].matches("abc-236"));
        assert!(tickets[0].matches("dark mode"));
        assert!(tickets[1].matches("READY"));
        assert!(!tickets[1].matches("dark mode"));
    }

    #[test]
    fn tickets_group_by_status_in_order() {
        let (tickets, _) = parse_task_page(PAGE).expect("parses");
        let groups = by_status(&tickets);
        assert_eq!(groups.iter().map(|(status, _)| *status).collect::<Vec<_>>(), ["building", "ready"]);
    }

    #[test]
    fn the_user_and_workspaces_are_read() {
        assert_eq!(parse_user_id(r#"{"user": {"id": 4242, "username": "j"}}"#).expect("parses"), 4242);
        let teams = parse_workspaces(r#"{"teams": [{"id": "9001", "name": "Acme", "members": []}]}"#).expect("parses");
        assert_eq!(teams, [("9001".to_owned(), "Acme".to_owned())]);
    }
}
