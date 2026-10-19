#!/bin/sh
# `pnpm demo`: Saturn on a made-up project, to look around or show it off. Agents in every state,
# ClickUp tickets in the right panel (made up, never sent anywhere), the database diagram, the
# history, uncommitted changes and their impact, a type error for the language server.
#
# Nothing of yours is touched: the demo has its own settings folder and its own project, in
# $TMPDIR/saturn-demo, made again on every run.
set -eu
cd "$(dirname "$0")/.."

home="${TMPDIR:-/tmp}/saturn-demo"
sh scripts/demo/make-project.sh "$home"
cargo build --release -p saturn

SATURN_DEMO_SHELL="${SHELL:-/bin/zsh}" \
SHELL="$PWD/scripts/demo/shell.sh" \
SATURN_USER_DATA="$home/settings" \
SATURN_PROJECT="$home/acme-shop" \
SATURN_MORE_REPOS="$home/acme-docs" \
SATURN_PANES=4 \
SATURN_FAKE_AGENTS=1 \
SATURN_FAKE_TICKETS=1 \
SATURN_OPEN_FILE="$home/acme-shop/packages/money/src/tax.ts" \
  exec target/release/saturn
