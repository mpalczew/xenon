# Design: VS Code tasks.json in Xenon

Status: implemented. Spec: VS Code
[tasks](https://code.visualstudio.com/docs/editor/tasks),
[variables](https://code.visualstudio.com/docs/editor/variables-reference),
[appendix](https://code.visualstudio.com/docs/editor/tasks-appendix).

## Goals

- A workspace's `.vscode/tasks.json` works as it does in VS Code for every
  field that has a meaning outside VS Code's extension host.
- A task runs as its own process in a Xenon-owned terminal tab. It is never
  typed into the user's shell, so Xenon sees the exit status and can sequence
  tasks.
- Keyboard first: ⌘⇧R picks any task, ⌘⇧B runs the build task.
- Pure parse/resolve/orchestration logic in `xenon_core::tasks`; the app only
  spawns terminals and draws.

## Code map

| Piece | Where |
|---|---|
| JSONC parse, OS overrides, file-level globals | `xenon_core/src/tasks/parse.rs` |
| Typed definitions | `tasks/model.rs` |
| `${...}` variables | `tasks/vars.rs` |
| Command line, spawn args, secret redaction | `tasks/resolve.rs` |
| `dependsOn` state machine | `tasks/plan.rs` |
| Background readiness | `tasks/background.rs` |
| folderOpen closure and approval hash | `tasks/trust.rs` |
| Runs, tabs, inputs (model) | `xenon_ui/src/app/task_runs.rs`, `task_tabs.rs`, `task_inputs.rs`, `task_trust.rs`, `tasks.rs` |
| Task terminal (exit code, output watch) | `xenon_terminal/src/view/task_run.rs` |
| Approval store | `xenon_store/src/task_trust.rs` |
| Phone | `xenon_ui/src/app/remote/tasks.rs`, `xenon_remote`, `page.html` |

## Mapping: VS Code field to Xenon behaviour

| VS Code | Xenon |
|---|---|
| JSONC (comments, trailing commas) | `serde_json_lenient` |
| `label`, legacy `taskName` | `label` wins; `taskName` accepted. Duplicate labels: first wins, a warning row is listed |
| `detail` | Picker subtitle, after the command line |
| `hide` | Omitted from picker and phone list; still runnable as a dependency |
| `type: shell` | Spawns `options.shell.executable` (default `$SHELL`, else `/bin/bash`) with `options.shell.args` (default `-l -c`, so a Finder-launched Xenon sees the user's PATH) and one argument: the command line |
| `type: process` | `command` is exec'd directly with `args` |
| Other `type` (`npm`, `gulp`, `cargo`, ...) | Listed dimmed as "unsupported" with the reason; refused with a toast on run |
| `command`, `args` (string or `{value, quoting}`) | Shell tasks: command as written, args quoted (`strong` default, `weak`, `escape`) |
| Task with `dependsOn` and no command | Composite: done when its dependencies are |
| `options.cwd`, `options.env` | Expanded; relative cwd is under `${workspaceFolder}` |
| `osx` / `linux` / `windows` | Deep-merged over the task (and over the file level); `windows` ignored here |
| File-level `type`, `options`, `presentation`, `problemMatcher`, `isBackground`, `runOptions`, `command`, `args` | Inherited by each task; `options`, `presentation`, `env` merge key by key, everything else is replaced |
| Legacy `version: 0.1.0` (`isBuildCommand`, `isTestCommand`, `showOutput`, `echoCommand`, `suppressTaskName`) | Mapped to `group` and `presentation` |
| `group`, `isDefault` | Picker tag (`build · default`). ⌘⇧B runs the default build task, else the only build task, else opens the picker |
| `dependsOn` (string or array of labels), `dependsOrder` | Parallel by default; `sequence` chains siblings. Cycles, missing and unsupported dependencies are errors before anything starts. A failing dependency stops the chain |
| `dependsOn` by `{type, ...}` identifier | Unsupported (task is listed with the reason) |
| `isBackground`, `problemMatcher.background` (`activeOnStart`, `beginsPattern`, `endsPattern`; legacy `watching`) | A background dependency is ready when `endsPattern` matches (after `beginsPattern`, or from the start with `activeOnStart` or no begins pattern). |
| Built-in matcher names | `$tsc-watch` has real patterns. Every other name, and a background task with no patterns, is ready on its **first non-empty output line**. Problem matching itself is out of scope: Xenon has no Problems panel and nothing is wired into LSP diagnostics |
| `presentation.reveal` | `always` selects the new tab; `silent` adds it unselected and selects it if the task fails; `never` adds it unselected (Xenon has no hidden terminals) |
| `presentation.focus` | `true` moves focus to the task tab; otherwise focus stays where it was |
| `presentation.panel` | `shared`: reuse one task tab per workspace once its process has exited; `dedicated`: reuse the tab of the same label once exited; `new`: always a new tab. A still-running tab is never replaced |
| `presentation.clear` | Implied: a reused tab is a fresh terminal |
| `presentation.close` | The tab closes when the task exits with code 0 (failures stay) |
| `presentation.echo` | The command line (secrets blanked) is printed first, dimmed |
| `presentation.showReuseMessage` | Accepted, no effect (no "press any key" screen) |
| `runOptions.runOn: folderOpen` | See Trust |
| `inputs` `promptString` (`description`, `default`, `password`) | Modal prompt, password masked |
| `inputs` `pickString` (strings or `{label, value}`, `default`) | Modal filterable list |
| `inputs` `command` | Unsupported: any run that needs it is refused |
| `${workspaceFolder}`, `${workspaceFolder:name}`, `${workspaceFolderBasename}`, `${workspaceRoot}`, `${cwd}`, `${userHome}`, `${pathSeparator}`, `${execPath}`, `${defaultBuildTask}` | Workspace = the directory that holds `.vscode/`. `${cwd}` is that directory too |
| `${file}`, `${fileWorkspaceFolder}`, `${relativeFile}`, `${relativeFileDirname}`, `${fileBasename}`, `${fileBasenameNoExtension}`, `${fileExtname}`, `${fileDirname}`, `${fileDirnameBasename}`, `${lineNumber}`, `${selectedText}` | From the Mac's active editor pane, only for the workspace on screen. With no editor the run is refused with a clear message |
| `${env:NAME}` | Process environment; unset is empty (as VS Code) |
| `${config:key}` | Xenon settings, dotted keys, scalar values (`settings.json`). Unknown keys are errors |
| `${input:id}` | The answered input |
| `runOptions.instanceLimit`, `reevaluateOnRerun`, `promptOnClose`, `icon`, `statusbar`, `problemMatcher` (non-background) | Parsed and ignored |

Picker: Return runs the task as its presentation says. ⌘Return runs it in a new
tab of its own, revealed and focused, whatever its `panel`. Each row shows the
real command line (`$ cmd`), a `build · default` chip, and for unsupported
tasks the reason. Escape closes.

Task tabs are ordinary tabs: closing the tab kills the process. They are saved
in the layout like any terminal tab, so after a restart they come back as plain
shells.

## Run model

`start_task` (the Mac picker, ⌘⇧B, folderOpen and the phone all call it):
load and parse tasks.json, plan the dependsOn closure (`Orchestrator`), collect
`${input:...}` ids across the closure, ask them one at a time, then launch.
Each `Action::Launch` resolves one task against a `VarContext` and spawns a
terminal. Terminal events drive the orchestrator: process exit 0 or a
background task becoming ready satisfies a node; any other exit fails the run
(toast naming the task, "the dependsOn chain stopped" for a dependency).

## Trust

- **folderOpen never runs without consent.** The default is off. When a
  workspace opens and tasks.json has `runOptions.runOn: "folderOpen"` tasks that
  are not approved, a non-modal card appears (it takes no focus and no Escape)
  listing the exact command lines of those tasks and their dependencies, with
  Allow and Don't allow. ⌘⌥↩ allows and ⌘⌥⌫ declines, from anywhere while the
  card shows.
- The approval is keyed to a SHA-256 of the merged definitions of the folderOpen
  tasks and everything they depend on (every field that affects what runs,
  variables unexpanded). Editing tasks.json, or a `git pull` or agent doing so,
  asks again. Before saving a choice Xenon re-reads the file and compares the
  hash with what the card showed; if it moved, the card is redrawn instead.
- Approvals are stored in Xenon's own data dir (`task_trust.json`), never in
  the repo. A declined hash is remembered too, so it does not nag.
- The picker shows each task's real command line, so a misleading label cannot
  hide what runs.
- `promptString` with `password: true` is masked in the field, never copied from
  it, never written to logs, and never typed into the terminal: the value goes
  to the process in its argv or env as the task wrote it. The echoed command line
  and the picker blank secret values. (Argv is visible to other local processes;
  prefer `env` in tasks.json.)
- The phone cannot approve folderOpen: that stays a Mac-side prompt. A paired
  device can already type into any terminal, so letting it run tasks gives it no
  new privilege. See the phone section.

## Phone

Contract in `docs/designs/mobile-remote-v2.md`. A paired phone sees a **Tasks**
row next to New terminal in a workspace's terminal list. The list entries carry
label, detail, resolved command line, group/default and the unsupported reason.
Run is the same `start_task` as the Mac picker (same resolve, dependsOn,
presentation, task tab), against `services.main_window` and the workspace
asked for. A phone run never changes the Mac's selected tab or focus. When the
root task's terminal exists the Mac sends `taskStarted` and the page attaches to
it. Inputs are asked on the phone that started the run; the answer returns with
the token and is accepted only from that connection. A password is masked on the
page, cleared from the field once sent, never put in localStorage, redacted in
`Debug` output of the message, and handed to the process as the Mac does.

## Unsupported, on purpose

Extension-contributed task types and `command` inputs; `dependsOn` by task
identifier; Problems panel and problem matchers (other than readiness); remote
(SSH) workspaces (tasks run on this Mac); `windows` overrides; `${command:...}`;
per-task `instanceLimit`; terminal-reuse messaging.
