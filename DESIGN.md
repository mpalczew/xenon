# DESIGN.md

Visual system for the Xenon agent shell. Complements
`PRODUCT.md`. Implementation: `crates/xenon_ui/src/chrome.rs` and consumers.

## Register

Product UI. Familiar IDE bones; signature is multi-stream anti-IDE chrome.

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

`theme.status().warning` static dot on a workspace or terminal tab that needs
the user (bell or inferred idle-after-output). `theme.status().info` pulsating
dot while that terminal is in an agent-sized burst. Working outranks attention
on the same tab; the workspace row is the aggregate of its terminals. Not
hard-coded hex. Pulse is status, not decoration.

## Typography

System UI for chrome labels. Medium weight on active chips/rows. Mono only in
editor/terminal content (settings-controlled faces).

## Motion

None for selection. Instant paint. Speed is product. Exception: the working
status pip uses a quiet opacity pulse (~1.4s, theme token).

## Themes

Bundled families under `crates/xenon_terminal/assets/`. Default dark: **Neon Noir**.
Default light: **One Light** (white canvas, not a gray slab). Opt-in void: **True Black**.
Theme gallery: ⌘⌥T, separate from Settings.

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
