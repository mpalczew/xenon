# DESIGN.md

Visual system for the Xenon agent shell. Complements `PRODUCT.md`. The implementation is
`crates/xenon`: stock GTK 4 and libadwaita widgets that follow the desktop's light or
dark look, plus a small CSS sheet.

## Register

Product UI. Familiar IDE bones; signature is multi-stream anti-IDE chrome.

## Selection language (mandatory)

Never background-only. Always ≥2 channels:

| Surface              | Fill              | Type                   | Edge                 |
| -------------------- | ----------------- | ---------------------- | -------------------- |
| Tab                  | selected tab fill | text vs muted + weight | accent underline     |
| Stream / tree / list | selected row fill | text vs muted + weight | left bar when active |
| Toolbar toggle       | selected          | text vs muted          | —                    |

## Attention

The warning color is a static status rail on an unselected terminal tab that needs the
user (bell or inferred idle-after-output). The info color is a pulsing status rail on an
unselected terminal tab while it is in an agent-sized burst. Selected tabs have no
status rail; the bottom accent underline is selection-only. Working outranks attention
on the same tab; the workspace row is the aggregate of its terminals.

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

Paper (not built in GTK yet) is a document surface, not chrome: `#f8f4ec` page and
`#2b2722` ink in light, `#16140f` and `#ddd5c6` in dark, with honey selection (`#edb950`
light, `#73571e` dark; the theme fill disappears on the page), on by default for
Markdown and applied to all three modes. No full screen mode: chrome stays put. ⌘+ / ⌘−
/ ⌘0 size the prose body (Settings → Writing size) while the prose pane has keys; in
Source they size the editor font.

## Motion

Selection is instant paint. Sidebar section open/close uses a 180ms spatial reveal with
6px travel. Unselected working workspace/tab rails use a quiet ~1.4s opacity pulse;
finished attention uses a static warning rail. Selected workspace and tab rails do not
carry status.

## GTK frontend

`crates/xenon` draws the product on GTK 4, libadwaita, VTE and GtkSourceView. The rule
is stock widgets plus one small CSS sheet (`theme.rs`); Xenon-owned drawing code is
zero.

- **Theme.** The desktop's. Colors, accent and light or dark come from libadwaita and
  follow `AdwStyleManager`. The editor and diff use the Adwaita or Adwaita-dark
  GtkSourceView scheme; the terminal uses a fixed light or dark palette. There is no
  theme picker.
- **Layout.** A header bar, a resizable sidebar (Workspaces, Files, Git) and the pane
  tree as nested `GtkPaned`. A focused pane gets a 2px accent top edge only when the
  layout has more than one pane.
- **Tabs.** `AdwTabBar`. The pane model, not the widget, owns tab order. Working tabs
  and workspaces pulse with the info color and tabs that need you show the warning
  color.
- **Sidebar.** Stock list rows. A workspace is a compact two-line row (name, `~` path)
  with a status line in the left margin and a git dirt tick on the right (filled: tree
  differs from HEAD; hollow: only the index does). Git shows status badges, change bars
  and a "Clean" line when there are no changes.
- **Diff tab.** Read-only, with GtkSourceView code views and stock text views for the
  line-number gutters. Unified below 860px, side by side at or above it, or pinned with
  Auto / Split / Unified (the model decides in `xenon_git`). Added and removed lines are
  paragraph tints.
- **Palettes.** One shell (`palette.rs`), an `adw::Dialog` with a search field and a
  list. Focus stays in the field; the models in `finder.rs` and `picker.rs` decide the
  rows.
- **Hints.** Every header button and similar control shows the chord of the action it
  runs, read from the key table in `keys.rs` (`App::chord`, `App::tip`), as `Label ·
  Ctrl+Shift+O`. Never typed by hand.
- **Empty screen.** An `AdwStatusPage` with the Xenon icon, buttons and the "Useful
  shortcuts" list.
- **Find.** A `GtkSearchEntry` strip above the terminal with VTE's own match highlight.
  Escape closes it and returns to the terminal.
- **Toasts.** `AdwToastOverlay` for results. Dialogs are `AdwAlertDialog`; Open
  workspace is the native `GtkFileDialog`.
- **Keys.** Accelerators are one table in `keys.rs`, with no entry for the keys the
  focused terminal hands to the shell (Ctrl+C, X, B, E, O, S). Ctrl+Alt+B toggles the
  sidebar because Ctrl+B belongs to the shell.
- **Markdown prose mode.** A `.md` file opens as writing, not source: the same text view
  with text tags (headings 1.7/1.4/1.2/1.08 times the body, bold, italic, struck, code
  chip, links in the accent color, a tinted quote and code block, accent list marks and
  task boxes), a column fitted to the file (the wrap width of a hard-wrapped file, else
  66 characters) centered by the view's side margins, and the line height as extra
  pixels between lines. Markers (`#`, `**`, `[`, fences, `>`) are the dim ink under
  Markers: Dim, or the `invisible` tag under Hide, except in the block the caret is in.
  Pictures sit in a space under their line (an overlay, never text in the file).
  Ctrl+click opens a link; click or Ctrl+Enter flips a task box; Alt+1 / Alt+3 switch
  Prose and Source (the worklist file opens as its list view instead; Alt+4 returns to
  it). Not built yet: Paper surface, quote rule, round bullet glyphs, table grid, code
  highlighting, list continuation on Return.
- **Settings.** Its own window (Ctrl+,) with a page sidebar. Pages: Fonts, Editor (with
  Markdown writing), Agents (Agent skill, Terminal) and Phone Remote. Ctrl+1 to 4 jump
  to a page; Escape or Ctrl+W closes. No search.

- **Phone remote.** Settings has a Phone Remote page (switch with status line, network,
  address override, port, Pair a phone, paired devices). Pair a phone is a dialog:
  black-on-white QR with a quiet border, the six-digit code, the address, Copy link,
  Done; Ctrl+Shift+M toggles it. A tab the phone drives reads "title · phone".

## Anti-patterns

- Side-stripe decoration on cards (nav selection edge is allowed)
- Gradient text, glassmorphism
- Hard-coded chrome colors; use libadwaita's named colors
- Single-channel selected state
