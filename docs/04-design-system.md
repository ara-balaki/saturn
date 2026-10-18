# Design system

Saturn's look comes from Jonas's reference boards: black panels floating on a darker ground, soft
squircle corners, white selected pills, glossy gradient tiles, a dot-matrix halftone, diagonal lines,
and the brand gradient from orange `#FF902F` through purple `#723CEB` to `#4C11CE`, with yellow
`#FFEF63` as the signal.

Version 2 (October 2026) brought it closer to the brand's app and landing pages, checked against the
guides on type, colour, space and motion:

- **Tinted neutrals.** Every grey leans slightly toward the purple (OKLCH hue 292), and hovers are the
  next step up one ramp. `ink-muted` went from `#9a9a9a` (APCA Lc 47, too faint for tab names) to
  `#b2b2b9` (Lc 60).
- **Softer shapes.** Panes 20px, tabs and cards 14, tiles 10, dialogs 28; buttons and chips are pills.
- **Type with contrast.** Weights 300, 400 and 600 only. Big type is light and tight, with at most one
  gradient word per screen.
- **One gradient moment per screen**, with a glossy top light: the primary button, where you are.
  White text sits on the purple end; on the orange it would be 2.3:1.
- **Motion with a job.** Hovers fade in 150ms, a pane that needs you breathes slowly, nothing else
  moves.

- **Browsable reference:** the Saturn Design System artifact,
  https://claude.ai/artifact/RC4SBXYReNJprpjxDNapf3: brand book, tokens, every component with a live
  preview, and the full main window.
- **In this repo:** `design/tokens.json` (the tokens, same file as the artifact) and
  `design/components.css` (the component classes, `sat-*`, used by the artifact's previews).
  `crates/saturn-design` compiles the tokens into Rust constants, so a token changes in one place.

![The Saturn main window](images/saturn-window.png)

## What colour means

| Colour   | Means                                            | Where                                    |
| -------- | ------------------------------------------------ | ---------------------------------------- |
| Yellow   | **Needs you**: a permission prompt or a question | Pane ring, tab, status dot, Dock count   |
| Orange   | **Your turn**: the agent finished                | Pane ring, status dot                    |
| Purple   | The brand, and **working**                       | Focused pane ring, working dot, switches |
| Red      | An error                                         | Status dot with the error's words        |
| Gradient | Where you are; the one primary action            | The project tile, the primary button     |

Yellow is reserved: nothing else in the interface uses it.

## Nested corners

Inner radius = outer radius − padding, so nested corners stay concentric. The tokens are built for
it:

| Outer                        | Padding     | Inner                                   |
| ---------------------------- | ----------- | --------------------------------------- |
| `radius-xl` 20 (panel, pane) | `space-2` 6 | `radius-lg` 14 (tabs, tree rows, cards) |
| `radius-lg` 14 (card, tab)   | `space-1` 4 | `radius-md` 10 (tiles, inputs)          |
| `radius-md` 10 (tile, input) | `space-1` 4 | `radius-sm` 6 (keys)                    |
| `radius-2xl` 28 (dialog)     | 14          | `radius-lg` 14                          |
| `radius-2xl` 28              | 18          | `radius-md` 10                          |
| `radius-pill`                | any         | `radius-pill`                           |

A test in `crates/saturn-design` fails when one of these rows stops adding up.

When a new nesting doesn't land on a token, change the padding, not the radius.

## Fonts

Plus Jakarta Sans for the interface and JetBrains Mono for terminals and code, bundled with the app
so Saturn never loads fonts from the network. GPUI draws a variable font only at its default weight,
so `crates/saturn/assets/fonts` holds one static file per weight in use (Plus Jakarta Sans 300, 400,
600; JetBrains Mono 400, 700), instanced from the variable fonts with fontTools. Both are OFL 1.1
with no Reserved Font Name.
