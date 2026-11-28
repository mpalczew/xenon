# Header trim and Workspaces in the sidebar switcher

Status: accepted. Follows `docs/designs/sidebar-panels.md`.

## Problem

The header bar carries 15 buttons plus a builds badge; the user uses about half. The
sidebar shows Workspaces above a Files | Git switcher, so it has two navigation styles.

Constraints: every removed button's command keeps its shortcut and stays in the command
palette and context menus unless listed as deleted below. The worklist page's own Add
and Edit form (`worklist/capture.rs`) stays; only the global "Add to Worklist" entry
goes.

## Usage (caller's view)

```rust
// window.rs: the whole header.
for cmd in [ToggleSidebar, SplitRight, SplitDown, ParkPane, Copy, CopyClean, Paste] {
    bar.pack_start(&button(cmd));
}
app.sidebar.header.pack_start(&main_menu()); // Settings is already in it
bar.pack_end(&app.save_button);
for cmd in [RunTask, OpenWorklist] {
    bar.pack_end(&button(cmd));
}

// sidebar: one switcher for all three.
app.sidebar.show(SidebarPage::Workspaces);
```

## Shape

```text
header  [sidebar][split →][split ↓][park][copy][copy clean][paste]  title  [worklist][▶][save]
        (over the panes only; [☰] appears here at the end only while the sidebar is hidden)
pane    tabs … [terminal icon]        (was "+", same action: new terminal in this pane)

sidebar  adw::ToolbarView, full window height (split header, as in Builder)
├── adw::HeaderBar  [☰]  [ Workspaces | Files | Git ]   adw::InlineViewSwitcher, icons only
└── adw::ViewStack (vexpand)
    ├── Workspaces   the existing list, full height (no 300 px cap, no "WORKSPACES" title)
    ├── Files
    └── Git
```

- `SidebarPage` gains `Workspaces`; it becomes the default for new settings.
- `focus_sidebar` (Ctrl+3) focuses whatever page is showing.
- New Workspace and Open Workspace move from the sidebar's + menu to the main menu
  (Ctrl+Alt+N, Ctrl+O / Ctrl+Shift+O unchanged).
- Removed from the header only: Prev/Next Workspace, Cut, Go to File, New Terminal,
  Settings.
- Deleted outright:
  - The builds badge ("4 on an older build") and its popover. Tabs keep their
    older-build tooltip (`build_notes`).
  - The global Add to Worklist command (Ctrl+Shift+K) and the code only it used: the
    section picker and the remembered last section in `open_capture`. The capture form
    always opens on the list's current section or on an item being edited.

## Tradeoffs accepted

- We accept not seeing the workspace list while on Files or Git, for one consistent
  sidebar. Ctrl+Alt+Up/Down still switch workspaces.
- We accept losing the "older build" count, for a quieter header; the tab tooltip still
  says it.

## Alternatives considered

- Keep Workspaces on top (current): two navigation styles.
- Move the trimmed buttons into a "+" menu (Builder's add button): the user doesn't use
  them, so deleting is less code.

## Open questions and risks

- Resolved: no + by the switcher; New/Open Workspace live in the main menu.

## Next implementation step

Trim `header()` in `window.rs`, delete the builds badge, swap the pane's add icon, then
move Workspaces into the stack.

## As built

- `adw::ViewSwitcher` is meant for header bars; in the sidebar body it drew large tiles
  and clipped the Git badge. `adw::InlineViewSwitcher` (icons only) replaced it; it has
  no badge, so the Git changed-file count is gone.
- Split header: each side of the width `Paned` is an `adw::ToolbarView` with its own
  `adw::HeaderBar`; the content header shows start title buttons only while the sidebar
  is hidden.
