# SSH workspaces

Open Workspace (⌘⇧O), type `ssh://devbox/home/alex/project`, and press Enter.
`devbox` can be a host alias from `~/.ssh/config`. For a directory under the
remote home, use `ssh://devbox/~/project`.

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
