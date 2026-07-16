# DESIGN.md

Visual system for the Xenon (working tree: xero) agent shell. Complements
`PRODUCT.md`. Implementation: `crates/xero_ui/src/chrome.rs` and consumers.

## Register

Product UI. Familiar IDE bones; signature is multi-stream anti-IDE chrome.

## Color strategy

**Restrained.** Tinted neutrals from the active theme pack; one accent
(`text.accent`) for selection edges and links. Semantic warning for attention.

## Elevation (dark)

| Level | Role | One Dark example |
|-------|------|------------------|
| 0 | Editor / terminal canvas | `#000000` |
| 1 | Panel, tab bar, toolbar | `#14161c` |
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

One shell: `crates/xero_ui/src/palette/`. Surfaces that type-to-filter over a
scrollable list **must** use it (finder, workspace open, run task, command /
stream / help). Geometry: width 640, max-height 420 (480 tall), elevation-2
panel, `list_selection` rows, scrollable results (`flex_1` + `min_h_0` +
`overflow_y_scroll`). Do not clone scrim/panel/input/list per feature.

## Attention

`theme.status().warning` dot on streams that need the user. Not hard-coded hex.

## Typography

System UI for chrome labels. Medium weight on active chips/rows. Mono only in
editor/terminal content (settings-controlled faces).

## Motion

None for selection. Instant paint. Speed is product.

## Themes

Bundled families under `crates/xero_terminal/assets/`. Default dark: **One Dark**
(with elevation). Opt-in void: **True Black**. Personality pack unchanged
(Neon, Mithril, …).

## Anti-patterns

- Side-stripe decoration on cards (nav selection edge is allowed)
- Gradient text, glassmorphism
- Hard-coded chrome hex outside theme assets
- Single-channel selected state
