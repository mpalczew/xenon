# VS Code tasks

Xenon runs a workspace's `.vscode/tasks.json` the way VS Code does. Parsing, variables,
ordering and trust are in `crates/xenon_core/src/tasks`; the app only spawns and draws.

## Decisions

- **A task is its own process in its own tab,** never typed into your shell, so Xenon
  sees its exit status.
- **Only `shell` and `process` tasks run.** Other types, `command` inputs, problem
  matchers and `${command:...}` are listed dimmed and refused.
- **Every `${input:...}` in the whole chain is asked before anything starts.**
- **`runOn: folderOpen` never runs without consent.** The approval is a hash of the
  tasks and everything they depend on, so any edit to tasks.json (a pull, an agent) asks
  again. Approvals live in Xenon's data folder, never the repo. The phone can't approve.
- **Password inputs** are never logged, echoed or typed. They still reach the process
  through argv or env as the task wrote them, and argv is visible to other local users,
  so prefer `env`.
