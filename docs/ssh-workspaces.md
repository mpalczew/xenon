# SSH workspaces

Open Workspace (⌘⇧O): the query text is the whole state. There is no hidden
host mode.

- A plain query also lists matching hosts (concrete `Host` aliases from
  `~/.ssh/config`, following `Include`, plus hosts of past SSH workspaces) as
  `ssh://thinkpad/` rows with an "SSH" chip, below local matches. Tab or Enter
  on one writes `ssh://thinkpad/` into the field.
- `ss` + Tab writes `ssh://`.
- `ssh://thin` lists hosts whose names start with `thin`. Tab or Enter on a
  host writes `ssh://thinkpad/`; it does not open anything.
- `ssh://thinkpad/<rest>`: `<rest>` is relative to the remote home unless it
  starts with `/` (absolute) or `~`. Rows are the folders under the parent of
  `<rest>` whose names start with the last segment, then fuzzy name matches for
  that segment (like local discovery), without repeats. Past workspaces on that
  host come first.
  - Tab on a row completes the field in the style you typed, ending in `/`:
    `ssh://thinkpad/sr` becomes `ssh://thinkpad/src/`.
  - Enter opens the selected row. With a whole folder typed (`ssh://thinkpad/src/`,
    `ssh://thinkpad/`) and no arrow press, Enter opens that folder; arrow down to
    pick a child instead. With no rows, Enter opens what you typed.
  - `ssh://devbox/~/project` opens the remote home's `project`;
    `ssh://devbox//home/alex/project` is absolute. (A single slash is now
    relative to home, so `ssh://devbox/home/alex/project` means
    `~/home/alex/project`.)
- The folder listing renders as soon as it arrives; name-search matches join
  below it later. "Searching thinkpad…" stays under the rows until the search
  finishes (a search failure shows a "Couldn’t search" row under the listing).
  Name matches rank project roots and plain folders before folders inside a git
  repo (`~/src/xenon` before `~/src/xenon/crates/x/src`); nothing is hidden.
  While nothing has arrived yet, it shows "Searching thinkpad…" alone. If it cannot be reached
  (auth, host key, network, no python3) a disabled row says so; run
  `ssh thinkpad` once in a terminal and try again.

The host needs Python 3 and tmux. Establish key authentication and accept the
host key with `ssh devbox` first: background file operations use OpenSSH batch
mode. SSH configuration, agent keys, and jump hosts remain OpenSSH's concern.

Files and ⌘P browse remote paths. Opening a text file fetches it in the
background; ⌘S writes it remotely. Saves compare the content revision before
replacing the file and preserve its permissions. Concurrent changes produce
the editor's existing conflict controls. A dropped connection keeps edits in
memory and retries reads. Unsaved drafts are not restored after quitting Xenon.
Remote text files are limited to 16 MiB. Refresh ⌘P to discover newly created
files. Binary/image editing, remote language servers, git badges, Run Task,
Save As, and file creation/rename/deletion from the tree are not supported;
use the remote terminal for filesystem and git operations.

Each terminal attaches to its own tmux session on the host's `xenon` tmux
server (`tmux -L xenon`). SSH failures retry the same session; reopening saved
terminal tabs also reattaches. Closing a tab or workspace detaches and leaves
its remote processes running. Exiting the remote shell ends that session.
Sessions are ordinary tmux sessions and can be accessed outside Xenon with
`tmux -L xenon list-sessions` and `tmux -L xenon attach -t <session>`.

## Implementation

Host lookups (`discover`, `list_dirs`) are read-only helper operations that need
no workspace. They share the per-host control connection, are capped like local
discovery (depth 3, 20 results, 2,500 directories, 4 seconds), never follow
symlinks while searching, and skip hidden and build directories.

`xenon_ssh` runs a bundled, standard-library Python helper over system SSH for
file operations. No helper binary or global service is installed. File requests
share an OpenSSH control connection in `~/.xenon/ssh`; terminal connections
attach to the persistent tmux daemon. Terminal session identities are saved
with the pane layout. This uses tmux's terminal ownership and screen redraw
rather than implementing a second terminal multiplexer.

Remote workspace paths live in a virtual namespace keyed by workspace UUID.
No project mirror is written on the Mac, and remote workspaces are excluded
from local filesystem watchers, local language servers, and the local IDE
bridge. Remote helper paths are confined to the chosen workspace, including
symlink resolution. Files outside it need another workspace.

`scripts/health` includes helper checks with mocked filesystem/process access.
Screenshot scenes cover connecting, the remote file tree/editor, and unavailable
remote files. A live-host check should cover save/conflict, dropping and
reconnecting SSH, and reopening an app session with an agent still running.
