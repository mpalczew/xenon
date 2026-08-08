# Claude Code IDE protocol

How Claude Code discovers and drives an IDE. Xenon implements the IDE side.

**Sources:** live capture from VS Code (2026-07-07); cross-checked with
[coder/claudecode.nvim PROTOCOL.md](https://github.com/coder/claudecode.nvim/blob/main/PROTOCOL.md)
(OSS reverse-engineer of the same extension). Official VS Code/JetBrains plugins
are not cleanly OSS; Neovim + live capture are the inspectable references.

**Product note:** this bridge is *Claude-specific garnish*. Xenon’s core bet is
PTY harness interop (session continues on the real CLI). Zed-style ACP embeds
the agent in the IDE and breaks that interop; we do not follow that path.

## Discovery
- Lock file: `~/.claude/ide/<port>.lock` (dir 0700, file 0600). Filename stem IS
  the port.
- Contents:
  ```json
  {"pid":4165,"workspaceFolders":["/abs/path"],"ideName":"xenon",
   "transport":"ws","runningInWindows":false,"authToken":"<uuid>"}
  ```
- The IDE injects `CLAUDE_CODE_SSE_PORT=<port>` into its integrated terminal's
  environment. That (plus the matching lock file) is how the CLI finds the IDE.
- Neovim docs also set `ENABLE_IDE_INTEGRATION=true`; VS Code live capture did
  not require it. Harmless to set; Xenon should inject both for compatibility.

## Transport
- WebSocket at `ws://127.0.0.1:<port>`.
- Auth: request header `x-claude-code-ide-authorization: <authToken>`.
- Messages: MCP (JSON-RPC 2.0), one JSON object per WS text frame.

## Handshake
- Client -> `initialize` -> server result:
  ```json
  {"protocolVersion":"2024-11-05",
   "capabilities":{"tools":{"listChanged":true}},
   "serverInfo":{"name":"xenon","version":"0.1.0"}}
  ```
- Client -> `notifications/initialized` (no response).
- Client -> `tools/list` -> server returns the tool list below.
- Client -> `tools/call` {name, arguments} -> server returns
  `{"content":[{"type":"text","text":"..."}]}` (MCP standard).

## Tools the IDE exposes (exact schemas from VSCode)
- `openFile` (required: filePath, startText, endText; opt: preview,
  selectToEndOfLine, makeFrontmost) - open a file, optionally select a range.
- `openDiff` (required: old_file_path, new_file_path, new_file_contents,
  tab_name) - show a diff.
- `getDiagnostics` (opt: uri) - LSP diagnostics.
- `getCurrentSelection` / `getLatestSelection` () - active editor selection.
- `getOpenEditors` () - list open editors.
- `getWorkspaceFolders` () - workspace roots.
- `checkDocumentDirty` (filePath) / `saveDocument` (filePath).
- `close_tab` (tab_name) / `closeAllDiffTabs` ().
- `executeCode` (code) - Jupyter only; Xenon returns unsupported.

## Xenon scope

Implemented: discovery, authenticated WebSocket transport, handshake,
`tools/list`, `openFile`, and `getWorkspaceFolders`. Unsupported tools return
valid stub responses so the CLI remains usable.

The next useful protocol work is selection push, environment compatibility,
and deeper `openFile` behavior. `openDiff` is deliberately lower priority.
Do not turn this bridge into the primary session model: the PTY owns the
session, and skills, resume state, and harness configuration stay native to the
harness.
