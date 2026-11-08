# PRODUCT.md

Xenon is a fast, native Linux shell for running many coding agents at once.

## What it is

Xenon keeps your agents, terminals and files in one window, organized by workspace. It
is not a chat panel and it is not tied to one vendor.

## Who it is for

It is for people who run several agents across several checkouts and are tired of
juggling windows and tabs. Casual single-file editing is not the goal.

## How it works

- A workspace is a project folder plus its saved layout and tabs. Workspaces are the
  unit of work.
- Each workspace has a tree of panes, and each pane holds a stack of tabs. Opening a
  file adds a tab in the focused pane. Closing the last tab in a pane leaves the pane
  empty, showing its empty page.
- The optional sidebar lists workspaces, then Files, then Git for the active workspace.
- Files works like a normal file manager.
- Each workspace has one task list, for you and your agents.
- A command palette and a file finder reach everything from the keyboard.
- The phone remote lets you see and use your terminals from a phone.
- Links open in the default app.

## Look

Xenon uses the desktop's own libadwaita look. Mode (System, Light, Dark) can force light
or dark. There are no theme packs.

## Principles

- The active workspace and tab are always visible.
- Speed matters more than animation.
- Every flow works from the keyboard. The mouse is an extra.
- Empty states give a short hint and no tours.

## Not in scope

- A collaborative cloud IDE.
- Replacing any agent's own interface.
- A deep multi-provider control layer. Xenon stays a shell.
- An in-app browser.

## Limits

- Terminals are not restored after a restart. They reopen as new shells.
