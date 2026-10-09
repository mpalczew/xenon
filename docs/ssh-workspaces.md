# SSH workspaces

Open Workspace (⌘⇧O) lists your SSH hosts (concrete `Host` aliases from
`~/.ssh/config`, following `Include`, plus hosts of past SSH workspaces) with an
"SSH" chip. Type `think` to find `thinkpad`, then press Enter to step inside the
host. The field now reads "Search folders on thinkpad…" and the footer says
"on thinkpad".

Inside a host:

- Type a name (`xen`) to search the host's `~/src`, `~/dev`, `~/code`,
  `~/Projects`, `~/Developer` like local discovery; `~/src xen` searches under
  one folder.
- Type a path (`~/src/`, `/etc/`, `src/` relative to the remote home) to list
  its folders. Tab writes the selected folder into the field; Enter opens it.
- Past workspaces on that host rank first.
- Backspace on an empty field returns to the local list; Esc closes.
- "Searching thinkpad…" shows while the host answers. If it cannot be reached
  (auth, host key, network, no python3) a disabled row says so; run
  `ssh thinkpad` once in a terminal and try again.

`ssh://devbox/home/alex/project` typed directly still works as a shortcut
(`ssh://devbox/~/project` for the remote home).

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
