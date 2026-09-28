# DESIGN.md

Visual system for the Xenon agent shell. Complements
`PRODUCT.md`. Implementation: `crates/xenon_ui/src/chrome.rs` and consumers.

## Register

Product UI. Familiar IDE bones; signature is multi-stream anti-IDE chrome.

## Shared library

Reusable controls live in `crates/xenon_design_system` and are consumed by
`xenon_ui`, `xenon_editor`, and `xenon_terminal`. A routine control owns its
input registration, focus lifecycle, keyboard editing, clipboard, IME,
appearance, and dismissal where applicable. Feature views provide state and
handle semantic events; they do not recreate those GPUI mechanisms. The editor
and terminal canvases remain specialized direct-GPUI surfaces. The boundary is
checked by `scripts/check-ui-boundaries`. Theme-semantic selection and status
helpers remain part of the shared vocabulary.

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

One shell. `palette_overlay` owns the panel, scrim, and dismissal.
`query_row` owns the row: List primary when selected, Body muted when idle,
Control label for the path and hint, Code for the trailing chip, an accent
left edge, and accent runs on the matched letters. Hover does not replace the
selected fill. `palette_input` is the only navigation path: the field stops
Enter, Escape, Tab, Up, and Down, and the palette matches `PaletteInput::Navigate`.

Surfaces: file finder (⌘P), workspace open (⌘⇧O), run task (⌘⇧R), command
palette (⌘⇧P), keyboard help (⌘⇧/), and the parent step of new workspace.
The theme gallery uses the same shell and key path; its cards are not query
rows. Geometry: width 640, max-height 420 (480 tall), elevation 2, scrollable
results (`flex_1` + `min_h_0` + `overflow_y_scroll`). Do not clone the scrim,
panel, input, or list per feature.

## Attention

`theme.status().warning` is a static yellow status rail on an unselected terminal
tab that needs the user (bell or inferred idle-after-output). `theme.status().info`
is a pulsing status rail on an unselected terminal tab while it is in an agent-sized
burst. Selected tabs have no status rail; the bottom accent underline is
selection-only. Working outranks attention on the same tab; the workspace row is
the aggregate of its terminals. Not hard-coded hex.

## Typography

`xenon_design_system::Typography` owns eight semantic roles across native
chrome: screen title (24/1.2, semibold), section title (18/1.35, semibold),
body (14/1.5), list primary (14/1.35, semibold), supporting text (14/1.6,
muted), control label (12/1.35, semibold, muted), button (12/1.35, semibold),
and code (13/1.55, mono).
Sizes scale with the UI font size setting; UI roles use the selected UI face,
and code uses the selected editor face. Theme colors provide text and muted
text rather than fixed light-theme values. The editor and terminal canvases
retain their own settings-controlled faces and rendering.

## Action controls

`xenon_design_system::action_button` owns the primary, secondary, quiet, and
icon action targets, including the label and the button type role. Callers
pass the action. They do not set the button's font, size, or weight. The
variant's foreground replaces the role's text color. Destructive is a
filled danger action, sized to its label. Button type is 12px semibold. Controls are
not tab stops (see `docs/keyboard-first.md`); they activate through GPUI's
click event. Disabled actions do not register a click handler. `xenon_design_system::checkbox` owns the 18px
checked/unchecked control and its accessible label. Checking pops the box and
expands a brief accent ring; unchecking settles back. The macOS Reduce Motion
setting makes state changes instant. Feature views supply the action and state;
they do not draw checkbox glyphs or wire keyboard activation.
`selectable_row` owns selection fill, the left accent edge, and hover. Callers
supply the row content. `bullet_list` renders indented supporting lines at
22px per level. `OutlineView` edits one title and indented points: Enter
splits a point, Tab and Shift-Tab change depth, and Backspace at the start
outdents or joins. The caller validates and saves the text.

## Toasts

`xenon_design_system::ToastView` is the one transient notice. Each window has a
host (`toast_host`); `show_toast` targets the main window and `show_toast_in`
a specific one, so any crate can report a result without reaching the app.
The island hangs from the toolbar's bottom edge, centered over the main panel
(the Settings window hangs it from the top). It carries a glyph, a List-primary
title, an optional Control-label detail, and at most one action chip. The chip
shows the action's shortcut inside a ring that drains over the four-second
lifetime; hovering refills and holds it. Success and info expire; errors tint
with the theme's error color, keep a full ring, add a dismiss button, and stay
until dismissed, replaced, or their action runs. One toast shows at a time; a
new one replaces it. It drops in with a small overshoot and errors shake once;
Reduce Motion makes both instant. Copy lives with the feature
(`xenon_ui::app::toasts` for the shell) and is short, plain, and a little warm.
Form and dialog errors stay inline; toasts report results of commands.

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
