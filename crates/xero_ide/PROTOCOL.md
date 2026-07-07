# Claude Code IDE protocol (captured live from VSCode, 2026-07-07)

How Claude Code discovers and drives an IDE. xero implements the IDE side.

## Discovery
- Lock file: `~/.claude/ide/<port>.lock` (dir 0700, file 0600). Filename stem IS
  the port.
- Contents:
  ```json
  {"pid":4165,"workspaceFolders":["/abs/path"],"ideName":"xero",
   "transport":"ws","runningInWindows":false,"authToken":"<uuid>"}
  ```
- The IDE injects `CLAUDE_CODE_SSE_PORT=<port>` into its integrated terminal's
  environment. That (plus the matching lock file) is how the CLI finds the IDE.
  `ENABLE_IDE_INTEGRATION` is NOT used in practice.

## Transport
- WebSocket at `ws://127.0.0.1:<port>`.
- Auth: request header `x-claude-code-ide-authorization: <authToken>`.
- Messages: MCP (JSON-RPC 2.0), one JSON object per WS text frame.

## Handshake
- Client -> `initialize` -> server result:
  ```json
  {"protocolVersion":"2024-11-05",
   "capabilities":{"tools":{"listChanged":true}},
   "serverInfo":{"name":"xero","version":"0.1.0"}}
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
- `executeCode` (code) - Jupyter only; xero returns unsupported.

## xero MVP scope
- Implement discovery + handshake + tools/list.
- Wire `openFile` -> open in xero's editor. `getWorkspaceFolders` -> roots.
- Stub the rest with valid (often empty) responses so the CLI stays happy.
