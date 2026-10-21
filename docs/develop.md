# Develop

## Build from source

You need [rustup](https://rustup.rs) and [pnpm](https://pnpm.io) as well.

```sh
git clone https://github.com/itsjonasarden/saturn.git
cd saturn
pnpm install
pnpm install-app      # builds Saturn.app and puts it in /Applications
```

Rust's version is pinned in `rust-toolchain.toml`, and rustup installs it on the first build.
`pnpm dist:dmg` builds a disk image for Apple Silicon and Intel to share, and
`pnpm release 0.2.0` publishes a version: it tags it, builds the disk image, signs it with the
Sparkle key in your keychain, and puts it on GitHub with the feed.

```sh
pnpm demo             # Saturn on a made-up project, to look around or show it off
pnpm dev              # build, sign and run Saturn from the code
pnpm health           # format check, clippy, tests, unused deps, licences
pnpm dist             # build Saturn.app in target/bundle.noindex
```

Node is only used for commit hooks and for formatting Markdown and JSON. Commits follow
[Conventional Commits](https://www.conventionalcommits.org).

| Crate                    | What it holds                                                                |
| ------------------------ | ---------------------------------------------------------------------------- |
| `crates/saturn`          | The app: window, title bar, grid, sidebar, editor, views, settings           |
| `crates/saturn-terminal` | Shells in ptys, alacritty's engine, Claude Code's signals, the terminal view |
| `crates/saturn-core`     | Logic without UI: layout, agents, git, imports, schemas, the LSP client      |
| `crates/saturn-design`   | Design tokens compiled from [`design/tokens.json`](../design/tokens.json)    |
| `crates/saturn-bridge`   | Where the plugin's hooks reach Saturn: a small HTTP server on a Unix socket  |

More reading: [the design system](design-system.md) and [the Rust stack](rust-stack.md).
Want to change something? [AGENTS.md](../AGENTS.md) has the rules, for people and coding agents alike.

## Spikes and screenshots

`SATURN_PANES=12 SATURN_COMMAND="node scripts/tui-load.mjs 30 14" SATURN_PERF=1 cargo run -p saturn --release`
opens a busy grid and prints frame timings. With `--features capture`, `SATURN_SCREENSHOT=out.png`
saves what the window draws.
