# PRODUCT.md

Xenon is a fast, native Linux shell for running many coding agents at once. It is
written in Rust on GTK 4, libadwaita and VTE. It is licensed GPL-3.0-or-later and the
repo is public at `mpalczew/xenon`.

For the phone remote, see `docs/designs/mobile-remote-v2.md`. Shortcuts are in
`crates/xenon/src/keys.rs`.

## What it is

Xenon keeps your agents, terminals and files in one window, organized by workspace. It
is not a chat panel and it is not tied to one vendor. Any agent that runs in a terminal
works, such as Claude Code, Grok or Kimi, with no special setup. The agent runs in a
real terminal, so you can resume the same session from another terminal or machine.

## Who it is for

It is for people who run several agents across several checkouts and are tired of
juggling windows and tabs. They need to see at a glance which workspace and tab is
active, which agent needs them, and which files have unsaved changes. Casual single-file
editing is not the goal.

## How it works

- A workspace is a project folder plus its saved layout and tabs. Workspaces are the
  unit of work.
- Each workspace has a tree of panes, and each pane holds a stack of tabs. Terminals,
  editors, diffs and images are all tabs. Opening a file adds a tab in the focused pane.
  You split a pane from the toolbar, by dragging a tab to an edge, or from the keyboard.
  Closing the last tab in a split pane removes the split.
- Terminals are not restored after a restart. They reopen as fresh shells.
- The optional sidebar lists workspaces, then Files, then Git for the active workspace.
- Files works like a normal file manager: a context menu (new, rename, trash, copy path,
  open, show in file manager), drag to move, and drops from other apps copy. Delete
  always goes to the trash.
- Git shows changes for git folders. Enter on a row opens a read-only diff tab.
- Each workspace has one task list at `.xenon/worklist.md`, for you and your agents. It
  opens as a checkbox list or as plain Markdown.
- Each terminal tab shows when an agent needs you or is busy, and the workspace row
  summarizes its tabs. This is a guess from the terminal's output, so it can be wrong.
- A command palette and a file finder reach everything from the keyboard.
- The phone remote lets you see and use your terminals from a phone.
- Links open in the default app. There is no built-in browser.

## Look

Xenon uses the desktop's own libadwaita look, including its high-contrast setting.
Settings › Appearance › Mode (System, Light, Dark) can force light or dark. There are no
theme packs. Chrome favors named libadwaita colors, and status never relies on color
alone.

## Principles

- Status is always visible: active workspace, active tab, attention, unsaved changes.
- Speed matters more than animation.
- Every flow works from the keyboard. The mouse is an extra.
- Empty states give a short hint and no tours.
- Nothing assumes one model vendor.

## Not in scope

- A collaborative cloud IDE.
- Replacing any agent's own interface.
- A deep multi-provider control layer. Xenon stays a shell.
- An in-app browser.
