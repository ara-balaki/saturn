//! Saturn's status line for Claude Code: the model, how full the context is, how far through its
//! task list the agent is, and how much of the plan's session it has used. Claude Code runs
//! `saturn status-line` with the session's state as JSON on stdin and shows what it prints under
//! the prompt, in Saturn's panes and in any other terminal.
//!
//! Installing it means setting `statusLine` in Claude Code's own settings, so the one it replaces
//! is kept, to put back when it's turned off.

use serde_json::{Value, json};

use crate::agent_activity::Steps;

/// The status line for Claude Code's input, with the agent's steps if it has a task list.
pub fn line(input: &Value, steps: Option<&Steps>) -> String {
    const DIM: &str = "\x1b[2m";
    const RESET: &str = "\x1b[0m";
    let mut parts = Vec::new();
    if let Some(model) = input.pointer("/model/display_name").and_then(Value::as_str) {
        parts.push(format!("{DIM}{model}{RESET}"));
    }
    if let Some(used) = input.pointer("/context_window/used_percentage").and_then(Value::as_f64) {
        parts.push(format!("{} {DIM}{:.0}%{RESET}", bar(used, 12), used.clamp(0., 100.)));
    }
    if let Some(steps) = steps {
        parts.push(format!("{DIM}step{RESET} {}/{}", (steps.done + 1).min(steps.total), steps.total));
    }
    if let Some(used) = input.pointer("/rate_limits/five_hour/used_percentage").and_then(Value::as_f64) {
        // Yellow as the session nears its limit, red at it: the one number here that can stop you.
        let tint = if used >= 95. {
            "\x1b[31m"
        } else if used >= 80. {
            "\x1b[33m"
        } else {
            ""
        };
        parts.push(format!("{DIM}session{RESET} {tint}{used:.0}%{RESET}"));
    }
    parts.join(&format!("  {DIM}·{RESET}  "))
}

/// A meter of `width` cells, filled to `percent`: blue on a dim track.
fn bar(percent: f64, width: usize) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a share of a few cells"
    )]
    let filled = ((percent.clamp(0., 100.) / 100.) * width as f64).round() as usize;
    format!("\x1b[34m{}\x1b[0m\x1b[2m{}\x1b[0m", "█".repeat(filled), "░".repeat(width - filled))
}

/// How far through its task list the agent is, from its transcript: the latest `TodoWrite` list,
/// or the tasks `TaskCreate` made and `TaskUpdate` moved on, whichever came last. `None` with no
/// list, or once every step is done.
pub fn steps_in_transcript(transcript: &str) -> Option<Steps> {
    #[derive(Clone)]
    struct Task {
        id: String,
        status: String,
        active: Option<String>,
    }
    let mut todos: Option<Steps> = None;
    let mut tasks: Vec<Task> = Vec::new();
    // Which list Claude touched last: true for tasks.
    let mut tasks_last = false;
    // Only lines that can matter are parsed: a long session's transcript runs to megabytes.
    let relevant = |line: &&str| line.contains("TodoWrite") || line.contains("Task") || line.contains("\"task\"");
    for line in transcript.lines().filter(relevant) {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(task) = entry.pointer("/toolUseResult/task")
            && let Some(id) = task.get("id").and_then(Value::as_str)
        {
            tasks.push(Task { id: id.to_owned(), status: "pending".to_owned(), active: None });
            tasks_last = true;
        }
        let uses = entry.pointer("/message/content").and_then(Value::as_array).into_iter().flatten();
        for tool in uses.filter(|item| item.get("type").and_then(Value::as_str) == Some("tool_use")) {
            let input = tool.get("input").unwrap_or(&Value::Null);
            match tool.get("name").and_then(Value::as_str) {
                Some("TodoWrite") => {
                    todos = crate::agent_activity::steps("TodoWrite", input);
                    tasks_last = false;
                }
                Some("TaskUpdate") => {
                    let Some(id) = input.get("taskId").and_then(Value::as_str) else { continue };
                    if let Some(status) = input.get("status").and_then(Value::as_str) {
                        if status == "deleted" {
                            tasks.retain(|task| task.id != id);
                        } else if let Some(task) = tasks.iter_mut().find(|task| task.id == id) {
                            status.clone_into(&mut task.status);
                        }
                    }
                    if let (Some(active), Some(task)) =
                        (input.get("activeForm").and_then(Value::as_str), tasks.iter_mut().find(|task| task.id == id))
                    {
                        task.active = Some(active.to_owned());
                    }
                    tasks_last = true;
                }
                _ => {}
            }
        }
    }
    let steps = if tasks_last && !tasks.is_empty() {
        let done = tasks.iter().filter(|task| task.status == "completed").count();
        let current = tasks.iter().find(|task| task.status == "in_progress").and_then(|task| task.active.clone());
        Some(Steps { done, total: tasks.len(), current })
    } else {
        todos
    };
    steps.filter(|steps| steps.total > 0 && steps.done < steps.total)
}

/// Whether Claude Code's settings run Saturn's status line.
pub fn installed(claude_settings: &Value) -> bool {
    claude_settings
        .pointer("/statusLine/command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.ends_with(" status-line") && command.contains("saturn"))
}

/// Points Claude Code's settings at `command` for the status line, returning the one it replaces
/// (`Null` when there was none).
pub fn install(claude_settings: &mut Value, command: &str) -> Value {
    let previous = claude_settings.get("statusLine").cloned().unwrap_or(Value::Null);
    if let Some(settings) = claude_settings.as_object_mut() {
        settings.insert("statusLine".to_owned(), json!({ "type": "command", "command": command, "padding": 0 }));
    }
    previous
}

/// Puts back the status line Saturn's replaced, or none.
pub fn uninstall(claude_settings: &mut Value, previous: Value) {
    if let Some(settings) = claude_settings.as_object_mut() {
        if previous.is_null() {
            settings.remove("statusLine");
        } else {
            settings.insert("statusLine".to_owned(), previous);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                chars.by_ref().find(|&c| c == 'm');
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn shows_the_model_context_steps_and_session() {
        let input = json!({
            "model": { "display_name": "Opus 5.5" },
            "context_window": { "used_percentage": 25.4 },
            "rate_limits": { "five_hour": { "used_percentage": 34.0, "resets_at": 1 } },
        });
        let steps = Steps { done: 1, total: 3, current: None };
        assert_eq!(plain(&line(&input, Some(&steps))), "Opus 5.5  ·  ███░░░░░░░░░ 25%  ·  step 2/3  ·  session 34%");
        assert_eq!(plain(&line(&json!({ "model": { "display_name": "Fable 5.1" } }), None)), "Fable 5.1");
    }

    #[test]
    fn follows_the_task_tools_and_the_todo_list() {
        let create =
            |id: &str| format!(r#"{{"type":"user","toolUseResult":{{"task":{{"id":"{id}","subject":"Step {id}"}}}}}}"#);
        let update = |id: &str, status: &str| {
            format!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"TaskUpdate","input":{{"taskId":"{id}","status":"{status}","activeForm":"Doing {id}"}}}}]}}}}"#
            )
        };
        let transcript =
            [create("1"), create("2"), create("3"), update("1", "completed"), update("2", "in_progress")].join("\n");
        let steps = steps_in_transcript(&transcript).expect("a task list");
        assert_eq!((steps.done, steps.total, steps.current.as_deref()), (1, 3, Some("Doing 2")));

        let todo = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"TodoWrite","input":{"todos":[{"content":"a","status":"completed","activeForm":"A"},{"content":"b","status":"pending","activeForm":"B"}]}}]}}"#;
        let steps = steps_in_transcript(&format!("{transcript}\n{todo}")).expect("the todo list, written last");
        assert_eq!((steps.done, steps.total), (1, 2));

        let finished = [create("1"), update("1", "completed")].join("\n");
        assert_eq!(steps_in_transcript(&finished), None);
        assert_eq!(steps_in_transcript(r#"{"type":"user"}"#), None);
    }

    #[test]
    fn installs_over_another_status_line_and_puts_it_back() {
        let theirs = json!({ "type": "command", "command": "sh ~/.claude/statusline.sh" });
        let mut settings = json!({ "model": "opus", "statusLine": theirs.clone() });
        let previous = install(&mut settings, "'/Applications/Saturn.app/Contents/MacOS/saturn' status-line");
        assert_eq!(previous, theirs);
        assert!(installed(&settings));
        assert_eq!(settings["model"], "opus");
        uninstall(&mut settings, previous);
        assert_eq!(settings["statusLine"], theirs);

        let mut empty = json!({});
        let previous = install(&mut empty, "saturn status-line");
        uninstall(&mut empty, previous);
        assert_eq!(empty, json!({}));
    }
}
