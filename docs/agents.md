# Claude Code and Codex

Saturn runs [Claude Code](https://claude.com/claude-code) and
[Codex](https://developers.openai.com/codex). It finds which of them your Mac has, installed and
logged in (`claude auth status`, `codex login status`); Settings → Agents shows what it found. With
only one, Saturn uses it and asks nothing. With both, New agent, Best of N and the routine form have
a **Claude Code | Codex** switch that remembers your last pick.

Settings → Agents also says what starts each one: `claude` and `codex`, or an alias of your own
(`cdsk`, say). Saturn types what it needs after it (a first message, `--resume …`, Codex's
`-p saturn`), so everything it starts or resumes runs your way.

## What each gives Saturn

|                                               | Claude Code                    | Codex                                         |
| --------------------------------------------- | ------------------------------ | --------------------------------------------- |
| Working, needs you, your turn                 | ✓                              | ✓                                             |
| What it waits for, answered from the queue    | ✓                              | ✓ (in sessions Saturn starts)                 |
| Resume after a restart                        | `claude --resume …`            | `codex -p saturn resume …`                    |
| Search sessions, Best of N, routines, Also in | ✓                              | ✓                                             |
| Limits at the foot of the sidebar             | the session, the week, a model | every window the plan has (for Pro, the week) |
| Status line                                   | Saturn's own, with its share   | Codex's own items                             |
| Teach the repo, what the agent is told        | ✓                              | ✓                                             |
| Which prompt changed a file, in the review    | ✓                              | –                                             |
| Agent teams                                   | ✓                              | –                                             |

## Claude Code

Saturn doesn't change your Claude Code setup. Every shell it opens gets `SATURN_SOCKET`,
`SATURN_PANE_ID` and `CLAUDE_CODE_PLUGIN_DIRS`, so a `claude` started inside Saturn loads the small
plugin in [`plugin/`](../plugin) for that session only. Its hooks tell the pane what the agent is
doing (working, waiting for a permission, done, failed) and do nothing outside Saturn; Claude Code
also reports its state to the terminal itself. Run `claude` in any other terminal and it's exactly as
before. Saturn's status line (Settings, off to begin with) is the one thing that reaches other
terminals, and only if you turn it on.

## Codex

Codex reports its state in its terminal title (a turning spinner while it works,
`[ ! ] Action Required` while it waits for you), and Saturn reads that in any pane running `codex`,
whoever typed it.

The Codex sessions Saturn starts run with a profile of Saturn's, `~/.codex/saturn.config.toml`
(`codex -p saturn`), laid over your own `config.toml`, which Saturn never touches. In it Saturn sets
only its own keys, and keeps anything Codex writes there (the folders you trust, for one):

- `[tui] notification_method = "osc9"` and `notification_condition = "always"`: Codex says what it
  waits for (`Approval requested: npm test`) to the terminal, where Saturn reads it.
- `notify`: as each turn ends, Codex runs Saturn's `plugin/bin/codex-notify` with the session's id
  and last answer, which tells Saturn what to resume. If you have a `notify` of your own, Saturn
  leaves it alone and finds the session from Codex's session files instead.
- `[tui] status_line`, only with Status line in Codex turned on in Settings.

Saturn asks Codex for the plan's limits as Codex's `/status` does (`codex app-server`, which starts
no session and spends nothing), and shows every window the plan reports: for some plans the 5-hour
one and the week, for others, Pro among them, only the week. When Codex can't be asked (offline,
say), its limits aren't shown, as Claude's aren't. Saturn reads Codex's sessions in
`~/.codex/sessions` for Search sessions and Teach the repo. It doesn't use Codex's hooks, which
Codex runs only once each is trusted.

## One instruction file for both

Both agents read `AGENTS.md`: Codex always, Claude Code (from 2.1.277) when there's no `CLAUDE.md` or
`CLAUDE.local.md` in the folder or above it. So a repo for both keeps its instructions in `AGENTS.md`
alone; a `CLAUDE.md` beside it stops Claude Code reading it.
