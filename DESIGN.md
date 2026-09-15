# DESIGN.md

Visual system for the Xenon agent shell. Complements
`PRODUCT.md`. Implementation: `crates/xenon_ui/src/chrome.rs` and consumers.

## Register

Product UI. Familiar IDE bones; signature is multi-stream anti-IDE chrome.

## Shared library

Reusable chrome primitives live in `crates/xenon_design_system` and are consumed
by `xenon_ui`. Its public vocabulary is `SelectionPaint`, `list_selection`,
`tab_selection`, `accent_surface`, and theme-semantic status
colors. Feature composition remains in `xenon_ui`.

The workspace rail uses a compact two-line row: folder icon and workspace name
on the first line, with the shortened root path (`$HOME` rendered as `~`) below
it. Workspace names use the UI font at medium weight; paths use muted UI text.
The active row uses the accent surface plus a left edge that carries the working
pulse. Section headers remain clickable/collapsible, but their chevrons are
intentionally hidden to match the quiet reference shell. The full path remains
available as the row tooltip. The rail does not add a descriptive status bar
beneath the tabs.

Maintained preview: `visual-review/xenon-design-system.html` and
`visual-review/xenon-mocks.html`. Native evidence is captured by
`scripts/visual-tests`.

## Color strategy

**Restrained.** Tinted neutrals from the active theme pack; one accent
(`text.accent`) for selection edges and links. Semantic warning for attention.

## Elevation (dark)

| Level | Role | One Dark example |
|-------|------|------------------|
| 0 | Editor / terminal canvas, sidebar | `#000000` |
| 1 | Toolbar, tab bar | `#14161c` |
| 2 | Elevated menus / popovers | `#1c1f26` |
| Selected | List / chip fill | `element.selected` |

**True Black** flattens 0–1 to pure black (OLED opt-in). Selection must still
work via multi-channel chrome (not bg alone).

## Selection language (mandatory)

Never background-only. Always ≥2 channels:

| Surface | Fill | Type | Edge |
|---------|------|------|------|
| Tab | `tab_*` (fallback `element.selected`) | text vs muted + weight | accent underline |
| Stream / tree / list | `element.selected` / transparent | text vs muted + weight | left bar when active |
| Toolbar toggle | selected / panel | text vs muted | — |

Helpers: `chrome::tab_selection`, `chrome::list_selection`.

## Elevated palettes (cmd-p family)

One shell: `crates/xenon_ui/src/palette/`. Surfaces that type-to-filter over a
scrollable list **must** use it (finder, workspace open, run task, command /
stream / help). Geometry: width 640, max-height 420 (480 tall), elevation-2
panel, `list_selection` rows, scrollable results (`flex_1` + `min_h_0` +
`overflow_y_scroll`). Do not clone scrim/panel/input/list per feature.

## Attention

`theme.status().warning` is a static yellow status rail on an unselected terminal
tab that needs the user (bell or inferred idle-after-output). `theme.status().info`
is a pulsing status rail on an unselected terminal tab while it is in an agent-sized
burst. Selected tabs have no status rail; the bottom accent underline is
selection-only. Working outranks attention on the same tab; the workspace row is
the aggregate of its terminals. Not hard-coded hex.

## Typography

System UI for chrome labels. Medium weight on active chips/rows. Mono only in
editor/terminal content (settings-controlled faces).

## Motion

Selection is instant paint. Sidebar section open/close uses a 180ms spatial
reveal with 6px travel. Unselected working workspace/tab rails use a quiet
~1.4s opacity pulse; finished attention uses a static warning rail. Selected
workspace and tab rails do not carry status.

## Themes

Bundled families under `crates/xenon_terminal/assets/`. Default dark: **One Dark**.
Default light: **One Light** (white canvas, not a gray slab). Opt-in void: **True Black**.
Theme gallery: ⌘⌥T, separate from Settings. Picking a card fills that
appearance slot; it does not switch Mode.

| Job | Families |
|-----|----------|
| Trust / defaults | One, Nord, Solarized, VS Code, IntelliJ, Xcode, High Contrast |
| Brand / screenshots | Neon, Abyss, Tokyo, Runner |
| Warm breadth | Ember, Gruvbox, Ayu |
| Personality | Imperial, Mithril, Synthwave, Radioactive, Hot Dog Stand |

Neon Noir is the hero theme; Abyss/Nord are the restrained professional
alternatives; Runner/Tokyo Night are cinematic. Hot Dog Stand intentionally
stays light-only and doubles as a detector for hard-coded, unthemed chrome.
Daily dogfood favors cool blues/cyans/violets; Neon Noir, Mithril, and Imperial
are proven dark daily-driver choices.

## Anti-patterns

- Side-stripe decoration on cards (nav selection edge is allowed)
- Gradient text, glassmorphism
- Hard-coded chrome hex outside theme assets
- Single-channel selected state
