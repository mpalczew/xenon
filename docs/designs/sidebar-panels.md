# Sidebar: Builder-style panels instead of stacked sections

Status: accepted. Sketches: plain box (A, C), VS Code solver (B), Builder patterns in
stock libadwaita (D), libpanel throughout (E).

## Problem

The sidebar stacks three collapsible sections (Workspaces, Files, Git) in two nested
`gtk::Paned`s. Folds, resize flags and two saved heights are juggled by hand in `fill`,
`restore_sizes`, `fit_workspaces`, `reset_sizes` and `set_fold`. That code has needed
about six fixes and still squeezes sections on restart and animates by jumping.

GNOME Builder doesn't stack sections. Its sidebar is one frame showing one panel at a
time behind a switcher (checked in `ide-primary-workspace.ui`), so it has no section
sizing at all. The user asked for as much of Builder as simplifies Xenon.

Constraints: stock gtk4 0.11 and libadwaita 0.9; the Files right-click on its header
(`files/ops.rs`) and Git's "clean" state must keep working; session JSON and
`workspaces.json` are untouched.

## Usage (caller's view)

```rust
// git_section.rs: Git reports its state; the sidebar decides nothing else.
app.sidebar.git_page.set_visible(status.is_some());
app.sidebar.git_page.set_badge_number(status.map_or(0, |s| s.files.len() as u32));

// commands: FocusFiles / FocusGit show their page.
app.sidebar.show(SidebarPage::Files);

// persist.rs: the shown page is the only sidebar state saved.
s.sidebar_page = app.sidebar.page();
```

## Shape

```text
sidebar root: gtk::Box (vertical)
├── Workspaces header + list   natural height, capped at 300 px, always visible
├── adw::ViewSwitcher          (Narrow) Files | Git, badge = changed files
└── adw::ViewStack             vexpand: takes all remaining height
    ├── Files page             the existing tree
    └── Git page               the existing list; status page "Clean" when clean
```

- GTK's box does all sizing: Workspaces asks for its rows, the stack expands. No folds,
  no paned positions, no saved heights.
- `SidebarPage { Files, Git }` is the only new type; `Sidebar::show` and `page` are the
  only new methods.
- Builder patterns copied, not linked: the panel switcher (`adw::ViewSwitcher`) and the
  theme choice as three radio items in the main menu (`gio::PropertyAction` on
  `StyleManager::color-scheme`), replacing Settings → Appearance.
- Kept as is: the horizontal sidebar `Paned` (drag width), the header bar, the center
  panes.

## Synthesis decision

- **Base: D** (stock libadwaita, no dependency): the switcher deletes the sizing problem
  instead of solving it.
- **From E:** Workspaces stays always visible on top. Workspaces is Xenon's main
  navigation; Builder has no equivalent, so hiding it behind a switcher would cost more
  than it saves.
- **Rejected:**
  - E's libpanel grid for the center (about −1,700 lines) loses drag-to-split, resets
    column widths on restart, adds a build dependency, and needs a type-name hack on a
    private resizer. Worth its own `/architect` later if the center's tab code becomes
    the next pain.
  - D's `OverlaySplitView`: it would drop drag-resizing the sidebar width.
  - B's VS Code solver: about +190 lines to keep section drag.
  - libpanel `PanelPaned`: no size API on the paned, leftover goes to the last child.
  - Omnibar and statusbar: neither deletes code.

## Tradeoffs accepted

- We accept seeing Files or Git, not both, in exchange for deleting all section sizing.
- We accept a fixed 300 px cap on Workspaces in exchange for no Workspaces divider.
- We accept moving the theme from Settings to the main menu in exchange for deleting the
  Appearance page.
- Old `settings.json` keys (three folds, two heights, unused `light_theme`/`dark_theme`)
  are dropped; serde ignores them, so no migration.

## Alternatives considered

- **Plain box with three sections (A/C):** about −75 lines, keeps all three visible, but
  still needs a rule for which section takes spare height and loses section drag.
- **libpanel throughout (E):** deepest saving, but its private types and open drag bugs
  leak into our code.

## Open questions and risks

- Resolved: one of Files or Git at a time is fine; the theme goes in the main menu.
- Does the Narrow switcher's icon-over-label height look right in a 200 px sidebar?

## Next implementation step

Replace the inner Paned and Git/Files sections with the switcher and stack, and delete
the fold and size code.
