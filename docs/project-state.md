# Current project decisions and gotchas

Code and focused design documents are authoritative for implementation. This
file keeps only cross-cutting decisions and traps that are expensive to
rediscover.

## Current model

- A workspace is a checkout root plus a saved pane-tree session.
- There is no user-facing stream concept. Parallel agents use terminal tabs
  and/or multiple workspaces.
- Terminals and editors are peer surfaces in pane tab stacks. PTYs are not
  restored across restart; terminal slots reopen as fresh shells.
- The Files tree lives under Workspaces in the optional sidebar. When both are
  open, Workspaces sizes to content (or a user-pinned height); drag the split
  between sections to resize. Files takes the remainder.
- Attention is per terminal tab: static warning after BEL or inferred
  idle-after-output; pulsing info while that terminal is in an agent-sized
  burst. The latter is local inference, not an agent completion event. Working
  outranks a settled mark on the same tab. The workspace row aggregates its
  terminals (any working → working; else any attention → attention). Viewing
  or using a tab dismisses only that tab.
- The Claude IDE bridge is optional garnish. PTY session portability is the
  product boundary.
- No embedded web browser or webview tab. Terminal URLs and `.html` / `.htm` /
  `.pdf` hand off to the system default app. Markdown preview stays a GPUI
  document. Agent computer-use stays in the harness (Playwright, Chrome,
  browser MCP). Revisit an in-app preview only if GPUI grows an overlay-safe
  webview, or a time-boxed spike plus dogfood evidence that the system-browser
  hop is a weekly blocker.

## Load-bearing choices

- `.vscode/tasks.json` exposes only `project xenon` and `project install`.
  Build/test/fmt/lint/health run via cargo or `scripts/*`; do not invent
  extra `project` labels.
- Known open/closed workspaces outrank filesystem discovery. Workspace MRU is
  persisted; file MRU is an in-memory tiebreaker only.
- Cmd-P serves its cached index immediately and refreshes in the background.
  It still respects ignore rules and skips heavy directories.
- ⌘[ and ⌘] are browser-style surface/location history, not tab cycling.
- Editor and terminal find share UX but not implementation. Vim search is a
  separate modal flow with smartcase.
- The embedded Vim layer is intentionally good-enough, not a Neovim host.
  Operators compose with motions: `dd`/`yy`/`cc`/`>>` are the line motion `_`
  (count lines), not a special "delete this line." Prefix count and motion
  count multiply (`2dd` = 2 lines, `2dj` = 3, `2d3d` = 6). Headless Neovim is
  the test oracle when `nvim` is on PATH; it is not the runtime. Daily edits
  cover join (`J`/`gJ`), `D`/`C`/`Y`/`S`, indent (`>>`/`<<`/`=`), number bump
  (`Ctrl-a`/`x`), `zz`, and visual-block (`Ctrl-v` with multi-range paint and
  blockwise registers). Indent uses a fixed 4-space shiftwidth (no setting).
  Still missing muscle-memory extras such as case ops, `R`/`U`, rich
  motions/objects, marks, macros, and full virtualedit block quirks.
- Settings geometry and chrome preferences share `settings.json`; partial
  saves use the locked load-mutate-save boundary and preserve fields they do
  not own.
- Theme gallery (⌘⌥T) is a separate overlay from Settings. Picking a card
  writes that appearance slot only. It does not switch Mode, so a dark pick
  while in light stays in light. The gallery stays open until Escape.
  Settings Appearance is Mode segments plus the live theme name (opens the
  gallery). Window is 640×480.
- Sidebar paints `editor_background` (canvas). Toolbar stays `panel_background`.
- Lilex is bundled and the default mono. GPUI still lists `.ZedMono` /
  `.ZedSans` aliases; the pickers hide those unless the files exist.

## Gotchas

- GPUI typed text arrives through `EntityInputHandler`, not ordinary key-down
  handlers. Printable key handlers must return unhandled.
- Palette and content teardown must route focus to a live surface or the shell.
- A fresh PTY grid is tiny before first paint. Remote-only reopen must call
  `ensure_grid_size`.
- Markdown YAML metadata requires the pulldown-cmark metadata option; otherwise
  the closing fence can become a setext heading.
- Markdown emphasis highlighting depends on `injection.include-children` and
  styled text runs, not just a grammar and colors.
- Overlay scrollbars require caret-follow calculations to reserve the bar.
- Git dirt stays flush-right; hover actions must occupy the same gutter.
- A completed `project install` updates the app on disk, but a running process
  keeps the old binary. Never kill Xenon to force the switch.
- In this codebase, “browser” is the Files tree, not a web view.
- GPUI (pinned Zed rev) has no webview primitive. The window is one Metal
  `NSView`. A native WKWebView / wry child view composites above the GPU
  scene, so palettes and menus draw behind the page. Zed’s wry experiment
  (PR 52447) hit this and did not merge. Do not add a webview surface unless
  upstream GPUI composites under overlays.

## Open product work

- Check [Rust Glancer](https://rust-glancer.github.io/blog/hello-world/) as a low-RAM disk-cached Rust LSP for `xenon_lsp` (<100MB, no proc-macro execution). HN: https://news.ycombinator.com/item?id=49393052
- Find replace.
- Keyboard paths for terminal On-exit, open-target choice, and workspace
  reorder.
- Public packaging left in `docs/release.md` (Homebrew, notarization,
  screenshots)—repo is already `mpalczew/xenon` and public.
