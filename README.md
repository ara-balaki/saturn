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

### Spikes and screenshots

`SATURN_PANES=12 SATURN_COMMAND="node scripts/tui-load.mjs 30 14" SATURN_PERF=1 cargo run -p saturn --release`
opens a busy grid and prints frame timings. With `--features capture`, `SATURN_SCREENSHOT=out.png`
saves what the window draws.
