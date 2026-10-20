//! Saturn's logic without any UI: the terminal grid as data, what an agent is doing, how to read
//! the signals Claude Code sends to its terminal, and the file tree with its git state. Everything
//! here is unit-tested.

pub mod agent_activity;
pub mod agent_state;
pub mod clickup;
pub mod dbt;
pub mod file_icon;
pub mod files;
pub mod format;
pub mod fuzzy;
pub mod git;
pub mod git_graph;
pub mod imports;
pub mod layout;
pub mod lsp;
pub mod program_status;
pub mod pull_request;
pub mod schema;
pub mod servers;
pub mod session_name;
pub mod shared_env;
pub mod shell_env;
pub mod status_line;
pub mod usage;
