# Design system

Saturn's look comes from Jonas's reference boards: black panels floating on a darker ground, big
squircle tiles, white selected pills, a dot-matrix halftone, dashed hairlines, and the brand gradient
from orange `#FF902F` through purple `#723CEB` to `#4C11CE`, with yellow `#FFEF63` as the signal.

- **Browsable reference:** the Saturn Design System artifact,
  https://claude.ai/artifact/RC4SBXYReNJprpjxDNapf3: brand book, tokens, every component with a live
  preview, and the full main window.
- **In this repo:** `design/tokens.json` (the tokens, same file as the artifact) and
  `design/components.css` (the component classes, `sat-*`). Phase 1 generates the app's CSS variables
  and the xterm.js theme from `design/tokens.json`, so a token changes in one place.

![The Saturn main window](images/saturn-window.png)

## What colour means

| Colour   | Means                                            | Where                                          |
| -------- | ------------------------------------------------ | ---------------------------------------------- |
| Yellow   | **Needs you**: a permission prompt or a question | Pane ring, tab, status dot, Dock count         |
| Orange   | **Your turn**: the agent finished                | Pane ring, status dot                          |
| Purple   | The brand, and **working**                       | Primary button, focused pane ring, working dot |
| Red      | An error                                         | Status dot with the error's words              |
| Gradient | Where you are; context used                      | The active IconTile, the ContextMeter          |

Yellow is reserved: nothing else in the interface uses it.

## Nested corners

Inner radius = outer radius − padding, so nested corners stay concentric. The tokens are built for
it:

| Outer                             | Padding     | Inner                                          |
| --------------------------------- | ----------- | ---------------------------------------------- |
| `radius-lg` 14 (panel, pane)      | `space-1` 4 | `radius-md` 10 (tabs, tree rows, header tiles) |
| `radius-lg` 14                    | `space-3` 8 | `radius-sm` 6                                  |
| `radius-md` 10 (tile, tab, input) | `space-1` 4 | `radius-sm` 6                                  |
| `radius-xl` 22 (dialog)           | 12          | `radius-md` 10                                 |
| `radius-pill`                     | any         | `radius-pill`                                  |

When a new nesting doesn't land on a token, change the padding, not the radius.

## Fonts

Plus Jakarta Sans for the interface and JetBrains Mono for terminals and code. Both are bundled with
the app in Phase 1 (`@fontsource`), so Saturn never loads fonts from the network.
