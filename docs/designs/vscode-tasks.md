# VS Code tasks

Xenon runs a workspace's `.vscode/tasks.json` the way VS Code does, for every field that
means something outside VS Code's extension host. Pure parsing, variable expansion,
ordering and trust logic is in `xenon_core/src/tasks`. The GTK side
(`xenon/src/tasks*.rs`) only spawns terminals and draws. Approvals are stored by
`xenon_store/src/task_trust.rs`.

## How it behaves

- Ctrl+Shift+R opens a picker of all tasks. Ctrl+Shift+B runs the default build task,
  else the only build task, else opens the picker. In the picker, Return runs a task as
  its `presentation` says, and Ctrl+Return runs it in a new tab that is revealed and
  focused. Each row shows the real command line, a `build · default` tag, and for
  unsupported tasks the reason.
- A task is its own process in a task tab. It is never typed into the user's shell, so
  Xenon sees the exit status. Closing the tab kills the process. Task tabs are saved
  with the layout and come back as plain shells after a restart.
- `shell` tasks run `options.shell.executable` (default `$SHELL`, else `/bin/bash`) with
  `-l -c` and the command line. `process` tasks exec the command directly.
- The file is read as JSONC. `linux` overrides are merged over the task and the file
  level; `osx` and `windows` are ignored.
- `dependsOn` runs in parallel by default, or in order with `dependsOrder: sequence`.
  Cycles, missing tasks and unsupported tasks are errors before anything starts. A
  failing dependency stops the run.
- A background task (`isBackground`) counts as done once launched, and its exit does not
  fail the run.
- Before a run starts, every `${input:...}` in the whole dependency chain is asked, one
  at a time, in a modal prompt. Password inputs are masked.
- `${file}`-style variables come from the active editor pane and only work for the
  workspace on screen. With no editor the run is refused. `${workspaceFolder}` and
  `${cwd}` are the directory that holds `.vscode/`.
- Duplicate labels: the first wins and a warning row is listed. `hide` removes a task
  from the picker and the phone list, but it still runs as a dependency.
- `presentation.reveal`, `focus`, `panel` and `close` are honored (a still-running tab
  is never reused). `echo` prints the command first. `clear` is implied by tab reuse.

## Not supported, on purpose

Task types other than `shell` and `process` (they are listed dimmed and refused on run),
`command` inputs, `dependsOn` by identifier, problem matchers and a Problems panel,
`${command:...}`, `instanceLimit`, and the "reuse" message. These fields are parsed and
ignored.

## Trust

- `runOn: folderOpen` never runs without consent. When a workspace opens with unapproved
  folderOpen tasks, a card that takes no focus lists the exact command lines (including
  dependencies), with Allow and Don't allow. Ctrl+Alt+Return allows and
  Ctrl+Alt+Backspace declines, from anywhere while the card shows.
- An approval is a SHA-256 of the merged definitions of those tasks and everything they
  depend on, with variables unexpanded. Any edit to tasks.json (a `git pull`, an agent)
  asks again. Before saving a choice Xenon re-reads the file and redraws the card if the
  hash moved.
- Approvals live in Xenon's data directory, never in the repo. A decline is remembered
  too.
- Password inputs are never logged or typed into the terminal, and the echoed command
  and picker blank them. The value still reaches the process through its argv or env as
  the task wrote it, and argv is visible to other local processes, so prefer `env`.
- Phone-started runs skip nothing: folderOpen approval happens only on the computer.
