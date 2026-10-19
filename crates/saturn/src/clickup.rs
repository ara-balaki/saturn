//! The ClickUp connection: a personal API token kept in the macOS Keychain, and the few calls
//! Saturn makes with it. Everything here blocks, so callers run it off the main thread. Without a
//! token, nothing about ClickUp shows anywhere.

use std::time::Duration;

use saturn_core::clickup::{self, Ticket};

const KEYCHAIN_SERVICE: &str = "Saturn ClickUp";
const KEYCHAIN_ACCOUNT: &str = "api-token";
const API: &str = "https://api.clickup.com/api/v2";
/// ClickUp pages team tasks 100 at a time; more than this many pages of open tickets assigned to
/// one person is not a list anyone picks from.
const MAX_PAGES: usize = 5;

/// The saved token, if one was set.
pub fn token() -> Option<String> {
    // A run with its own settings folder (the demo, a spike) is a test: it leaves your real token
    // alone, and doesn't make the Keychain ask about it.
    if std::env::var_os("SATURN_USER_DATA").is_some() {
        return None;
    }
    security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .filter(|token| !token.trim().is_empty())
}

pub fn save_token(token: &str) -> Result<(), String> {
    security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, token.trim().as_bytes())
        .map_err(|error| format!("The Keychain refused the token: {error}"))
}

pub fn forget_token() {
    let _ = security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
}

fn agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// `GET {API}{path}` with the token; ClickUp's error message on failure.
fn get(agent: &ureq::Agent, token: &str, path: &str) -> Result<String, String> {
    let mut response = agent
        .get(&format!("{API}{path}"))
        .header("Authorization", token)
        .call()
        .map_err(|error| format!("Could not reach ClickUp: {error}"))?;
    let status = response.status().as_u16();
    let body =
        response.body_mut().read_to_string().map_err(|error| format!("ClickUp's answer was cut off: {error}"))?;
    match status {
        200..=299 => Ok(body),
        401 => Err("ClickUp did not accept the token.".to_owned()),
        429 => Err("ClickUp asks to slow down; trying again in a minute.".to_owned()),
        _ => Err(format!("ClickUp answered {status}.")),
    }
}

/// Who the token belongs to and their first workspace, to check a token when it is pasted.
pub fn check(token: &str) -> Result<String, String> {
    let agent = agent();
    let workspaces = clickup::parse_workspaces(&get(&agent, token, "/team")?).map_err(|e| e.to_string())?;
    let (_, name) = workspaces.into_iter().next().ok_or("The token sees no ClickUp workspace.")?;
    Ok(name)
}

/// The open tickets assigned to the token's owner, in every workspace it sees.
pub fn my_tickets(token: &str) -> Result<Vec<Ticket>, String> {
    let agent = agent();
    let user = clickup::parse_user_id(&get(&agent, token, "/user")?).map_err(|e| e.to_string())?;
    let workspaces = clickup::parse_workspaces(&get(&agent, token, "/team")?).map_err(|e| e.to_string())?;
    let mut tickets = Vec::new();
    for (workspace, _) in workspaces {
        // Tasks name their space only by id; the names come from the space list, once.
        let spaces: std::collections::HashMap<String, String> = get(&agent, token, &format!("/team/{workspace}/space"))
            .ok()
            .and_then(|json| clickup::parse_spaces(&json).ok())
            .unwrap_or_default()
            .into_iter()
            .collect();
        let first = tickets.len();
        for page in 0..MAX_PAGES {
            let path = format!(
                "/team/{workspace}/task?assignees[]={user}&include_closed=false&subtasks=true&order_by=updated&page={page}"
            );
            let (mut found, more) = clickup::parse_task_page(&get(&agent, token, &path)?).map_err(|e| e.to_string())?;
            tickets.append(&mut found);
            if !more {
                break;
            }
        }
        for ticket in &mut tickets[first..] {
            ticket.space = ticket.space_id.as_ref().and_then(|id| spaces.get(id)).cloned();
        }
    }
    Ok(tickets)
}

#[cfg(test)]
mod tests {
    /// Reaches the real ClickUp with a token it can't accept: the connection, the endpoint and the
    /// error message, without an account. Needs the network, so it runs on request.
    #[test]
    #[ignore = "talks to api.clickup.com"]
    fn a_wrong_token_is_turned_away_politely() {
        assert_eq!(super::check("pk_0_not_a_real_token"), Err("ClickUp did not accept the token.".to_owned()));
    }
}
