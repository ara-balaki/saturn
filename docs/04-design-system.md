# Design system

Saturn follows the Quarn workflow-automation design (Behance, 2026) with blue in place of its green: near-black and grey almost everywhere, colour only where it has a job, and depth from light and blur instead of decoration.

- **Browsable reference:** the Saturn Design System artifact, https://claude.ai/artifact/RC4SBXYReNJprpjxDNapf3: brand book, tokens in both themes, every component with a live preview, and the full main window.
- **In this repo:** `design/tokens.json` (the tokens, same file as the artifact) and `design/components.css` (the component classes the previews use). `crates/saturn-design` compiles the tokens into Rust constants with a dark and a light value each, so a token changes in one place.

![The Saturn main window](images/saturn-window.png)

## What colour means

| Colour  | Means                                      | Where                                              |
| ------- | ------------------------------------------ | -------------------------------------------------- |
| Blue    | Alive: an agent working, focus, the action | Working dots, the focused pane, the primary button |
| Amber   | **Needs you**: a permission prompt         | Pane glow, a hidden tab, the title-bar count       |
| Neutral | **Your turn**: the turn finished           | A bright check and ring; done is calm              |
| Red     | An error                                   | With the error's words                             |

Amber is the only warm colour, so a pane that needs you is seen from across the room.

## One structure for every container

Every region is a **panel** (`surface-raised`, radius 10, 4px padding) holding a **strip** of controls and a **well** (`surface`, radius 6) with the content: a terminal, the editor's text, a list. Controls that belong together sit in a **track**, the title bar's included. The ground is plain; the dot grid only fills empty wells.

## Corners, from the window inward

Inner radius = outer radius − inset. The scale starts at the one corner Saturn does not draw, the macOS window (about 16):

| Outer                 | Inset | Inner                              |
| --------------------- | ----- | ---------------------------------- |
| the window, about 16  | 6     | panels, 10                         |
| panel or track, 10    | 4     | controls (tabs, buttons, wells), 6 |
| dialog or palette, 14 | 8     | controls, 6                        |

A test in `crates/saturn-design` fails when one of these stops adding up, and the design system's previews are checked corner by corner with a script.

## Density

Development tools need room for content, so controls are 28px tall, panels sit 6px apart and pad 4px.

## Fonts

Urbanist for the interface and JetBrains Mono for terminals and code, bundled so Saturn never loads fonts from the network. GPUI draws a variable font only at its default weight, so `crates/saturn/assets/fonts` holds one static file per weight in use (Urbanist 300, 400, 600; JetBrains Mono 400, 700), instanced from the variable fonts with fontTools. Both are OFL 1.1 with no Reserved Font Name.
