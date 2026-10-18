# Saturn

A native macOS app for running many Claude Code agents side by side: a grid of terminals with tabs,
a light editor, and a clear signal when an agent needs you. Written in Rust with GPUI.

- What it is and why: [docs/01-overview.md](docs/01-overview.md)
- The stack: [docs/05-rust-stack.md](docs/05-rust-stack.md)
- The design system: [docs/04-design-system.md](docs/04-design-system.md)

## Develop

Rust is pinned in `rust-toolchain.toml` (rustup installs it on first build). Node is only used for
commit checks and formatting Markdown/JSON.

```sh
pnpm install          # commit hooks and formatters
pnpm dev              # build and run Saturn (cargo run -p saturn)
pnpm health           # format check, clippy, tests, unused deps, licences
```

| Crate                    | What it holds                                                                |
| ------------------------ | ---------------------------------------------------------------------------- |
| `crates/saturn`          | The app: window, title bar, grid, shortcuts                                  |
| `crates/saturn-terminal` | Shells in ptys, alacritty's engine, Claude Code's signals, the terminal view |
| `crates/saturn-core`     | Pure logic: grid layout, agent states, OSC 7501 parsing                      |
| `crates/saturn-design`   | Design tokens compiled from `design/tokens.json`                             |

## Claude Code integration

Every pane's shell gets `SATURN_SOCKET`, `SATURN_PANE_ID` and `CLAUDE_CODE_PLUGIN_DIRS`, so a Claude
session started in Saturn loads the plugin in [`plugin/`](plugin) for that session only; nothing
is installed into Claude Code's own settings. The plugin's hooks report to the Saturn window the
session runs in, and do nothing anywhere else.

Settings (⌘,), saved in `~/Library/Application Support/Saturn/settings.json`:

- **Name tabs after Claude's session** (on by default): tabs show the session's name. Off: the
  folder, or a name typed by hand.
- **Name sessions after tickets** (on by default): `/start abc-224`, or any slash command whose
  first word is a ticket, renames the session to `ABC-224`, as `/rename` would. A name you gave
  yourself is never replaced.

### Spikes and screenshots

`SATURN_PANES=12 SATURN_COMMAND="node scripts/tui-load.mjs 30 14" SATURN_PERF=1 cargo run -p saturn --release`
opens a busy grid and prints frame timings. With `--features capture`, `SATURN_SCREENSHOT=out.png`
saves what the window draws.
