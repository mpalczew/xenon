# DESIGN.md

Visual system for the Xenon agent shell. Complements `PRODUCT.md`. Implementation:
`crates/xenon_ui/src/chrome.rs` and consumers.

## Register

Product UI. Familiar IDE bones; signature is multi-stream anti-IDE chrome.

## Shared library

Reusable controls live in `crates/xenon_design_system` and are consumed by `xenon_ui`,
`xenon_editor`, and `xenon_terminal`. A routine control owns its input registration,
focus lifecycle, keyboard editing, clipboard, IME, appearance, and dismissal where
applicable. Feature views provide state and handle semantic events; they do not recreate
those GPUI mechanisms. The editor and terminal canvases remain specialized direct-GPUI
surfaces. The boundary is checked by `scripts/check-ui-boundaries`. Theme-semantic
selection and status helpers remain part of the shared vocabulary.

The workspace rail uses a compact two-line row: folder icon and workspace name on the
first line, with the shortened root path (`$HOME` rendered as `~`) below it. Workspace
names use the UI font at medium weight; paths use muted UI text. The active row uses the
accent surface plus an accent left edge. Agent status is a separate short line in the
margin left of the row (blue and pulsing while working, yellow when it needs you; it
shows on the active row too). Git dirt is a short centered tick on the right edge in the
theme's `version_control.modified` color (pulled darker on light themes): filled when
the working tree differs from HEAD, hollow when only the index does, absent when clean.
Hover actions replace the tick in the same gutter. Section headers remain
clickable/collapsible, but their chevrons are intentionally hidden to match the quiet
reference shell. The full path remains available as the row tooltip. The rail does not
add a descriptive status bar beneath the tabs.

Maintained preview: `visual-review/xenon-design-system.html` and
`visual-review/xenon-mocks.html`. Native evidence is captured by `scripts/visual-tests`.

## Git section and diff tab

The sidebar's third section, under Files, for the active workspace. Header: GIT, branch,
and a file-count pill that stays visible when collapsed. Rows: round tinted status
badges (one for the index letter and one for the working-tree letter when both are set,
so `MM` shows two), file name colored by change kind (modified, added, deleted struck
through, renamed with its source, untracked), dimmed directory, and a five-cell
green/red change bar. A clean tree shows a two-line card: Clean, then Matches HEAD and
the branch. The row under the keyboard cursor uses the standard selection edge and fill.

The diff tab is read-only: a header with the path, `HEAD → working tree`, an Auto /
Split / Unified control, `+N -M` totals, and a Read-only lock. Rows use `diff.plus` /
`diff.minus` for signs and a 14% tint for line fills; changed words inside a paired line
get the theme's word-added / word-deleted color. Tree-sitter colors the code. Auto is
split at 860px of pane width and up. When HEAD and the working tree match but the index
differs, two labeled sections show Staged (HEAD | Index) and Unstaged (Index | Working
tree).

## Color strategy

**Restrained.** Tinted neutrals from the active theme pack; one accent (`text.accent`)
for links and tab underlines. Workspace-row selection edges use neutral `text` so they
never read as the blue working or yellow needs-you line. Semantic warning for attention.

## Elevation (dark)

| Level    | Role                              | One Dark example   |
| -------- | --------------------------------- | ------------------ |
| 0        | Editor / terminal canvas, sidebar | `#000000`          |
| 1        | Toolbar, tab bar                  | `#14161c`          |
| 2        | Elevated menus / popovers         | `#1c1f26`          |
| Selected | List / chip fill                  | `element.selected` |

**True Black** flattens 0–1 to pure black (OLED opt-in). Selection must still work via
multi-channel chrome (not bg alone).

## Selection language (mandatory)

Never background-only. Always ≥2 channels:

| Surface              | Fill                                  | Type                   | Edge                 |
| -------------------- | ------------------------------------- | ---------------------- | -------------------- |
| Tab                  | `tab_*` (fallback `element.selected`) | text vs muted + weight | accent underline     |
| Stream / tree / list | `element.selected` / transparent      | text vs muted + weight | left bar when active |
| Toolbar toggle       | selected / panel                      | text vs muted          | —                    |

Helpers: `chrome::tab_selection`, `chrome::list_selection`.

## Elevated palettes (cmd-p family)

One shell. `palette_overlay` owns the panel, scrim, and dismissal. `query_row` owns the
row: List primary when selected, Body muted when idle, Control label for the path and
hint, Code for the trailing chip, an accent left edge, and accent runs on the matched
letters. Hover does not replace the selected fill. `palette_input` is the only
navigation path: the field stops Enter, Escape, Tab, Up, and Down, and the palette
matches `PaletteInput::Navigate`.

Surfaces: file finder (⌘P), workspace open (⌘⇧O), run task (⌘⇧R), command palette (⌘⇧P),
keyboard help (⌘⇧/), and the parent step of new workspace. The theme picker is not a
palette (see Themes). Geometry: width 640, max-height 420 (480 tall), elevation 2,
scrollable results (`flex_1` + `min_h_0` + `overflow_y_scroll`). Do not clone the scrim,
panel, input, or list per feature.

## Attention

`theme.status().warning` is a static yellow status rail on an unselected terminal tab
that needs the user (bell or inferred idle-after-output). `theme.status().info` is a
pulsing status rail on an unselected terminal tab while it is in an agent-sized burst.
Selected tabs have no status rail; the bottom accent underline is selection-only.
Working outranks attention on the same tab; the workspace row is the aggregate of its
terminals. Not hard-coded hex.

## Typography

`xenon_design_system::Typography` owns eight semantic roles across native chrome: screen
title (24/1.2, semibold), section title (18/1.35, semibold), body (14/1.5), list primary
(14/1.35, semibold), supporting text (14/1.6, muted), control label (12/1.35, semibold,
muted), button (12/1.35, semibold), and code (13/1.55, mono). Sizes scale with the UI
font size setting; UI roles use the selected UI face, and code uses the selected editor
face. Theme colors provide text and muted text rather than fixed light-theme values. The
editor and terminal canvases retain their own settings-controlled faces and rendering.

## Action controls

`xenon_design_system::action_button` owns the primary, secondary, quiet, and icon action
targets, including the label and the button type role. Callers pass the action. They do
not set the button's font, size, or weight. The variant's foreground replaces the role's
text color. Destructive is a filled danger action, sized to its label. Button type is
12px semibold. Controls are not tab stops (see `docs/keyboard-first.md`); they activate
through GPUI's click event. Disabled actions do not register a click handler.
`xenon_design_system::checkbox` owns the 18px checked/unchecked control and its
accessible label. Checking pops the box and expands a brief accent ring; unchecking
settles back. The macOS Reduce Motion setting makes state changes instant. Feature views
supply the action and state; they do not draw checkbox glyphs or wire keyboard
activation. `xenon_design_system::switch` is the 30×18 on/off control for settings that
apply immediately; checkboxes stay for picking items. The knob travels in 180ms with a
small overshoot while the track crossfades; Reduce Motion makes it instant.
`selectable_row` owns selection fill, the left selection edge (`border.selected`;
neutral `text` on workspace rows), and hover. Callers supply the row content.
`bullet_list` renders indented supporting lines at 22px per level. `OutlineView` edits
one title and indented points: Enter splits a point, Tab and Shift-Tab change depth, and
Backspace at the start outdents or joins. The caller validates and saves the text.

## Settings window

A 200px sidebar (panel → background gradient) holds search and seven pages; nav rows use
`selectable_row`, with an accent "On" pill on Phone Remote while the remote runs. The
pane shows the page's rows as one card: Body name, Control-label detail, control on the
right. The focused row gets a hover fill and a 2px accent edge only once the keyboard is
driving. Appearance, Fonts, and Editor lead with a 168px live preview (rail · editor ·
terminal at 11/14 of real sizes) with a soft accent glow and a LIVE pill. Theme swatches
show editor background, accent, and keyword color; the current one carries an accent
ring.

## Empty states

`xenon_design_system::all_clear` fills an empty pane with a centered column: a 64px
accent-framed check, a Screen title, and a Supporting line. Callers append `key_chip`
pills (keycap plus Control label; clickable when it acts). On mount the check wipes in
over 700ms after a 300ms rest, then a ring expands for 900ms; an accent glow behind it
breathes on a 5s loop. Reduce Motion and visual captures show the finished state. The
worklist uses it when empty.

## Markdown prose

Markdown files open in Prose: you type in the rendered view and the file text is the
only document. Modes sit in a quiet status strip under the editor (counts left, Paper
and Full, then Prose / Split / Source with their keys); the active one carries ink
color, semibold, and a tint. Split shows source left and prose right over the same
buffer, caret and scroll following.

Prose reads in a centered column 66 characters wide (settings: 40 to 120) in a serif
(Auto: serif on Paper, the interface face otherwise), 18px at 1.55 line height. Syntax
stays as real text in a dim ink at least 3:1 on the page, never hidden until the caret
arrives: heading `#`s hang in the left margin, list markers sit in the indent, quote
marks hang left of a 2px rule and the quote is italic, a trailing two-space break shows
a dim return arrow. Task boxes are checkboxes (click or ⌘Return flips one character;
done items are struck and muted). Code and tables are monospace on a tint, table pipes
dim, fences highlighted by language. `---` draws a hairline. An image alone in a
paragraph shows below its line (dashed placeholder when it will not load). Markers: Hide
draws only the result (bullets as dots, task boxes alone, table separator as a line).

Leading `<!-- -->` lines fold into one chip ("3 comments", with a flag mark when one
says FLAG); click or Return toggles it, remembered per file.

Paper is a document surface, not chrome: `#f8f4ec` page and `#2b2722` ink in light,
`#16140f` and `#ddd5c6` in dark, with honey selection (`#edb950` light, `#73571e` dark;
the theme fill disappears on the page), on by default for Markdown and applied to all
three modes. No full screen mode: chrome stays put. ⌘+ / ⌘− / ⌘0 size the prose body
(Settings → Writing size) while the prose pane has keys; in Source they size the editor
font.

## Toasts

`xenon_design_system::ToastView` is the one transient notice. Each window has a host
(`toast_host`); `show_toast` targets the main window and `show_toast_in` a specific one,
so any crate can report a result without reaching the app. The island hangs from the
toolbar's bottom edge, centered over the main panel (the Settings window hangs it from
the top). It carries a glyph, a List-primary title, an optional Control-label detail,
and at most one action chip. The chip shows the action's shortcut inside a ring that
drains over the four-second lifetime; hovering refills and holds it. Success and info
expire; errors tint with the theme's error color, keep a full ring, add a dismiss
button, and stay until dismissed, replaced, or their action runs. One toast shows at a
time; a new one replaces it. It drops in with a small overshoot and errors shake once;
Reduce Motion makes both instant. Copy lives with the feature (`xenon_ui::app::toasts`
for the shell) and is short, plain, and a little warm. Form and dialog errors stay
inline; toasts report results of commands.

## Motion

Selection is instant paint. Sidebar section open/close uses a 180ms spatial reveal with
6px travel. Unselected working workspace/tab rails use a quiet ~1.4s opacity pulse;
finished attention uses a static warning rail. Selected workspace and tab rails do not
carry status.

## Themes

Ten bundled families under `crates/xenon_terminal/assets/`, one dark and one light each
(True Black is dark-only). Default dark: **One Dark**. Default light: **One Light**
(white canvas; every text color at least 4.5:1, which suits presbyopic eyes:
dark-on-light reads better for all ages, see Piepenbrock et al. 2013, *Ergonomics*). A
saved theme that is no longer bundled falls back to the default for its slot.

| Group     | Families                              |
| --------- | ------------------------------------- |
| Everyday  | One, Nord, Solarized, High Contrast   |
| Signature | Neon (hero), Tokyo, Mithril, Imperial |
| Warm      | Gruvbox                               |
| OLED      | True Black                            |

Neon Noir, Mithril, and Imperial are proven dark daily drivers. Hot Dog Stand is still
bundled but hidden from the picker: set it by hand to catch hard-coded, unthemed chrome.
New families must earn a slot against these; do not add variants (Soft/Hard/Storm) or
look-alikes of other editors.

**Theme panel (⌘⌥T).** A 340px panel docked on the right under the toolbar, no scrim, so
the app itself is the preview. One row per family with dark and light snippet tiles
painted in each theme's own colors; the current family carries an "in use" pill. ↑/↓ and
←/→ (or a click) re-theme the whole app live; Return or Keep stores that half in its
slot (Mode does not change), Escape, Revert, ⌘⌥T, or any palette opening puts back the
saved theme. Settings › Appearance shows five swatches for the on-screen slot and opens
this panel for the rest.

## GTK frontend

`crates/xenon_gtk` draws the same product on GTK 4, libadwaita and VTE (Linux). The rule
is stock widgets plus one CSS sheet; Xenon-owned drawing code is zero. Decisions:

- **Theme.** The saved theme (mode, light and dark family) is read from the same bundled
  theme files as the GPUI app and turned into one CSS sheet that overrides libadwaita's
  named colors (window, view, headerbar, popover, accent), so dialogs, toasts, the
  search entry and scrollbars follow the theme with no per-widget code. Terminal colors
  come from the theme's `terminal.*` keys. Mode System follows the OS through
  `AdwStyleManager`. Icons are pinned to Adwaita so symbolic icons exist on every
  desktop.
- **Layout.** Header bar (36px: sidebar toggle, title with the workspace name, split
  buttons), a resizable `GtkPaned` sidebar (`settings.sidebar_width`) and the pane tree
  as nested `GtkPaned`. A focused pane gets a 2px accent top edge only when the layout
  has more than one pane (the GPUI ring). A pane shows its tab strip only when it has
  more than one tab or the workspace has more than one pane; a lone terminal gets the
  whole window and the header subtitle names its tab.
- **Tabs.** A strip of chips (plain boxes and labels) in a scrolled window, not
  `AdwTabBar`: the status rail needs a widget we own, and the pane model, not the
  widget, owns tab order. A chip is label, close button on hover and selection, and a
  2px rail. Selected: accent rail, text weight 600, `bg` fill. Unselected: muted text;
  the rail is the warning color (static, needs you) or the info color (pulsing,
  working). Same rules as above: selected tabs carry no status.
- **Sidebar.** Two sections in a vertical `GtkPaned`. WORKSPACES is a `GtkListBox`: a
  folder icon, name semibold and the `~` path muted (cut at the start so the folder name
  stays), a 3px status line in the left margin, and for the active row the neutral-text
  left edge plus `element.selected` fill. FILES is a `GtkListBox` tree (chevron, icon,
  name; the open file is accent and semibold). No diff-status section yet.
- **Palettes.** One shell (`palette.rs`): a scrim over the window and a 640px panel at
  elevation 2 with a field, a `GtkListView` (matched letters in the accent color, detail
  and badge on the right) and a hint line. Focus stays in the field; the models in
  `finder.rs` and `picker.rs` decide the rows. Selection is element.selected plus a 2px
  accent left edge.
- **Titles.** Tab chips show the shared title cleanup: `user@host:path` prompts become
  the directory name, or the shell name (`bash · 1`) when it is the folder the terminal
  started in.
- **Pulse.** Working rails and workspace lines fade between five opacity steps over
  about 1.4s. One 10 Hz timer exists only while something is working; a frame is drawn
  only when the step changes (about 6 per second), and only the small rail widgets are
  dirty. Nothing else animates; there are no CSS keyframes, which would tick the frame
  clock at display rate.
- **Find.** A `GtkSearchEntry` strip above the terminal (reveal animation 120ms), VTE's
  own match highlight and wrap-around. Escape closes it and returns to the terminal.
- **Toasts.** `AdwToastOverlay` for results ("Opened x with the default app", "No room
  for another split"). Dialogs (new workspace) are `AdwAlertDialog`; Open workspace is
  the native `GtkFileDialog`. Empty and unsupported states are `AdwStatusPage`.
- **Motion, scrolling, text.** Smooth pixel scrolling in VTE, GTK's own HiDPI scaling
  and font rendering. Terminal font and size come from `settings.json` (a face that is
  not installed falls back to Monospace); zoom writes the size back.
- **Keys.** Accelerators are generated from `xenon_keymap` (Linux defaults plus the
  user's `keymap.json`), minus every key the focused terminal hands to the shell
  (Ctrl+C, X, B, E, O, S). Ctrl+Alt+B toggles the sidebar because Ctrl+B belongs to the
  shell.

- **Settings.** `AdwPreferencesDialog` (Ctrl+,): pages Appearance, Fonts, Editor, Terminal,
  Agents, Language Servers (Ctrl+1 to 6, Ctrl+F searches, Escape closes). Theme tiles are
  buttons painted by one generated sheet in each theme's own colors. The picker page
  re-themes the window as the selection moves; Enter keeps, Escape or Back reverts. The
  interface size scales the window font as a percentage of 14; the default size and the
  System face add no rule.

- **Phone remote.** Settings has a Phone Remote page (switch with status line, network,
  keep-awake, address override, port, Connect Phone, paired devices with Revoke). Connect
  Phone is a dialog: black-on-white QR with a quiet border, the six-digit code, the address,
  Copy link, Done; Ctrl+Shift+M toggles it. A flat footer line under the sidebar shows the
  state (listening, waiting for the other Xenon, "<phone> connected", "<phone> driving"), muted
  until a phone is connected, then accent. A tab the phone drives reads "title · phone".

## Anti-patterns

- Side-stripe decoration on cards (nav selection edge is allowed)
- Gradient text, glassmorphism
- Hard-coded chrome hex outside theme assets
- Single-channel selected state
