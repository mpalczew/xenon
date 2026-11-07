# Xenon feature inventory, cut candidates, and cleanup list

Written 2026-11-07 from `main` at `7711581`. Nothing here is a decision. The cut
candidates and the cleanup list are proposals for the user to accept or reject.

How the numbers were made: line counts are `wc -l` over `*.rs` (tests included) in the
named paths, rounded. "GTK status" is read from `crates/xenon_gtk` on `main`, plus two
unmerged branches named below. "Shared" means the code already lives in a GPUI-free
crate and the GTK app uses it.

The two unmerged branches:

- `worktree-agent-a6101b1c2f9016601`: Git section, diff tab, sidebar spacing, toolbar
  hints (+2.5k lines, mostly `xenon_gtk`).
- `worktree-agent-ad3978011b03424c7`: Markdown prose mode and a new `xenon_prose` crate
  (+2.4k lines).

## Where the weight is

| Part                                                                      | Rust lines | Notes                             |
| ------------------------------------------------------------------------- | ---------: | --------------------------------- |
| GPUI frontend: `xenon_ui`                                                 |     27,400 | App shell, all chrome             |
| GPUI frontend: `xenon_editor`                                             |     18,800 | Buffer, vim, prose, highlight     |
| GPUI frontend: `xenon_design_system`                                      |      4,500 | GPUI widgets                      |
| GPUI frontend: `xenon_terminal`                                           |      4,000 | View over Zed's terminal          |
| GPUI frontend: `xenon` (binary, visual test runner), `xenon_stub`         |      1,550 | Dock stub is macOS only           |
| Vendored Zed: `third_party/gpui_linux`, `third_party/terminal`            |     22,300 | Not ours, still ours to carry     |
| GTK frontend: `xenon_gtk`                                                 |      9,100 | Plus ~4.9k on the two branches    |
| Shared crates (core, store, remote, finder, git, lsp, ide, ssh, and more) |     15,000 | Excludes the 1,780-line page.html |

- The GPUI side is about 56k own lines plus 22k vendored. The GTK side plus shared
  crates is about 24k. Deleting GPUI removes roughly three quarters of the Rust.
- `cargo tree -p xenon` lists about 946 distinct crates. `cargo tree -p xenon_gtk` lists
  about 190. The difference is Zed's stack, fifty tree-sitter grammars, and macOS and
  X11 plumbing.
- `xenon_gtk` does not depend on `xenon_editor`, `xenon_terminal`, `xenon_ui`,
  `xenon_design_system`, `xenon_git` (until its branch merges), `xenon_lsp`,
  `xenon_ide`, or `xenon_memory`. It reaches `xenon_ssh` only through `xenon_core`.

## Feature inventory

Format: what it does, where it lives, GTK status, size. "GPUI" sizes are what gets
deleted or replaced; "GTK" sizes are what exists now.

### Workspaces and sidebar

- **Workspace registry and sessions.** A workspace is a folder plus a saved layout.
  Open, closed, and recent workspaces persist; the registry decides which workspace a
  file belongs to. Lives in `xenon_core` (`workspace.rs`, `session.rs`, `route.rs`,
  about 270 lines) and `xenon_store` (`lib.rs`, `settings.rs`, about 470). GTK status:
  done, shared.
- **Workspace rail.** The sidebar list of workspaces with active, agent-status, and
  git-dirt marks, collapse, and a Files section below. GPUI: `xenon_ui/src/sidebar/`,
  `app/workspaces.rs` (about 1,500 lines). GTK: `sidebar.rs`, `workspaces.rs`,
  `model.rs` (about 600 lines). Status: partial. Missing: rename, reorder, hover
  actions, git-dirt tick (on the git branch), spacing fix (on the git branch).
- **New / open workspace and pickers.** Ctrl+Shift+O picker with known workspaces,
  folder discovery under project roots, path completion, and `ssh://` completion. Also a
  "create new folder" flow. GPUI: `workspace_picker/`, `workspace_create.rs`,
  `app/workspace_create_ops.rs` (about 1,800 lines). GTK: `picker.rs`, `palette.rs`
  (about 800). Status: partial. Local picker is done; the `ssh://` half is missing.
  Shared rules in `xenon_finder::discover` (about 530 lines).
- **Files tree.** Lists the workspace folder, expands lazily, watches at most 48 open
  folders. GPUI: `file_browser.rs`, `app/browser.rs`, `app/browser_menu.rs`,
  `app/tree_keys.rs` (about 1,300). GTK: `files.rs`, `files_rows.rs` (about 550).
  Status: partial. Listing, expanding, and opening files work. Missing: the context menu
  (new file or folder, rename, move to trash, copy path, reveal), tree keyboard
  navigation parity.
- **Window state and boot.** Remembers window size and maximized state. GPUI:
  `app/boot.rs`. GTK: `persist.rs`, `window.rs`. Status: done.
- **Empty state.** Teaching text when nothing is open. GPUI: `app/empty_state.rs` (about
  300). GTK: `empty.rs` (about 50). Status: done (simpler).

### Panes and tabs

- **Pane tree.** Nested splits with fractions, tabs per pane, unsplit on last close,
  parked empty panes, depth limits. Lives in `xenon_core` (`pane_*.rs`, `content.rs`,
  `split.rs`, about 1,200 lines), generic over the frontend's tab type. GTK status:
  done, shared. GPUI glue (`app/split_ops.rs`, `content_ops.rs`, `resize.rs`,
  `render.rs`, about 1,700) goes away.
- **Tab bar.** Tab chips with status, dirty mark, close, overflow menu, context menu,
  middle-click. GPUI: `tabs/`, `app/tab_context.rs` (about 1,600). GTK: `pane.rs` (about
  280). Status: partial. Missing: overflow handling and tab context menu.
- **Tab drag and drop.** Drag a tab to move it (center) or split (edge). GPUI:
  `app/tab_drop.rs`. GTK: not started (worklist "Next").
- **Divider drag and keyboard resize.** GPUI: `resize.rs`. GTK: drag done in
  `layout.rs`; keyboard resize not started.
- **Back and forward history** (Ctrl+[ and Ctrl+]). GPUI: `app/nav_history.rs`,
  `app/nav_ops.rs` (about 300). GTK: not started.
- **Dirty-close prompt.** GPUI: `app/dirty_close.rs` (156). GTK: not started; closing a
  modified editor tab just closes it.
- **Toolbar.** GPUI: `toolbar.rs` (about 470). GTK: libadwaita header bar in
  `window.rs`; "old toolbar hints" are on the git branch. Status: partial.

### Terminal

- **Terminal tabs.** A real PTY per tab. GPUI: `xenon_terminal` (4,000 lines) on top of
  vendored Zed `third_party/terminal` (8,700 lines, plus a small Xenon hook). GTK:
  `term.rs` (about 200) on VTE. Status: done.
- **Find in terminal, links, zoom, copy and paste.** GTK uses VTE's search, match
  regexes, and clipboard: `find.rs`, `links.rs` (about 220). Status: done. VTE gives
  these for free; the GPUI equivalents are `find*.rs`, `view/paths.rs` (about 500).
- **Copy Clean.** Strips agent TUI chrome (quote bars, box lines) from a selection.
  GPUI: `xenon_terminal/src/clipboard.rs`, `bar_gutter.rs` (about 440). GTK: not
  started. The cleaning logic is pure text and should move into a GPUI-free crate.
- **Image paste and drag-in of paths.** GPUI: `clipboard.rs`, `view/render.rs` (about
  350). GTK: not started.
- **Terminal title cleanup and tab labels.** `xenon_core/src/tab_label.rs`. GTK status:
  done, shared.

### Agent status and attention

- **Working and attention marks.** A tab pulses while an agent-sized burst of output
  runs, and shows a static mark after a bell or inferred idle. Workspace rows aggregate
  their tabs. Lives in `xenon_core` (`attention.rs`, `attention_map.rs`, `activity.rs`,
  `agent.rs`, about 250). GTK: `model_status.rs`, `events.rs` (about 380). Status: done,
  shared. GPUI still carries its own copy of the burst rule in
  `xenon_terminal/src/view/mod.rs` (lines 202 to 210, 377, 450 to 470).
- **Pulse animation.** GPUI: `xenon_design_system/src/pulse.rs`. GTK: `window.rs` pulse
  source at 10 Hz steps. Status: done.

### Editor

- **Plain editing.** Open, edit, save, external-change detection, soft wrap, find. GPUI:
  `xenon_editor` core (`buffer/`, `edit.rs`, `undo.rs`, `selection.rs`, `find.rs`,
  `view/`, `element/`, about 9,000 lines). GTK: `editor.rs` (181 lines) on
  GtkSourceView, which provides undo, search, wrap, line numbers, and highlighting.
  Status: partial. No dirty-close prompt, no external-change reload prompt, no in-editor
  find bar yet. Files over 4 MB or not UTF-8 open in the default app.
- **Vim mode.** GPUI: `xenon_editor/src/vim/` (4,140 lines, pure, no GPUI). GTK: uses
  GtkSourceView's `VimIMContext` when the setting is on. Status: done by the toolkit.
  The custom engine is not ported and does not need to be.
- **Syntax highlighting.** GPUI: `highlight/` (780 lines) plus fifty tree-sitter grammar
  crates and `queries/`. GTK: GtkSourceView language definitions. Status: done by the
  toolkit.
- **Diff gutter and LSP.** Worklist "Next". Not started in GTK.
- **Image viewer.** GPUI: `image_viewer/`, `view/image.rs` (about 700). GTK: images fall
  through to the default app. Status: not started.
- **Ex commands** (`:w`, `:s`, and similar). GPUI: `vim/ex.rs`, `view/ex_io.rs` (about
  680). GTK: `:w` only, from GtkSourceView.

### Markdown

- **Prose mode** (Prose, Split, Source views of one buffer, Paper look, task boxes, link
  following). GPUI: `xenon_editor/src/prose/`, `view/prose/`, `markdown/` (about 4,000).
  GTK: a new `xenon_prose` crate plus `prose*.rs` and `editor_auto.rs` on the Markdown
  branch (about 1,900). Status: partial, unmerged. The GTK version is a rewrite on
  GtkTextTags and does not share code with the GPUI one.
- **Link and path-under-cursor follow.** GPUI: `markdown/link.rs` (324, GPUI-free). GTK:
  not started.

### Git

- **Git status for the sidebar** (branch, changed files, line counts, dirt tick).
  `xenon_git` (about 1,000 lines, GPUI-free). GTK: on the git branch (`git.rs`,
  `git_ops.rs`, `git_section.rs`, about 900). Status: partial, unmerged.
- **Diff tab** (side by side or unified, changed words). Model in `xenon_git::diff`.
  GPUI: `diff_view.rs`, `diff_rows.rs` (about 620). GTK: `diff_view.rs`, `diff_text.rs`
  on the git branch (about 900). Status: partial, unmerged.
- **Git-dirt watcher.** GPUI: `app/git_dirt.rs` (376). GTK: `git.rs` on the branch. Two
  copies of the same debounce and worker idea.

### Finder and pickers

- **Ctrl+P file finder.** Cached index, fuzzy ranking with nucleo, respects ignore
  rules. `xenon_finder` (1,150 lines). GTK: `finder.rs`, `palette.rs` (about 650).
  Status: done.
- **Command palette.** GPUI: `command_palette/`, `commands.rs` (about 480). GTK: not
  started (worklist "Next").
- **Theme picker.** GPUI only. GTK follows the system light and dark; no picker.

### Settings

- **Settings window.** Seven pages with search. Rules in `xenon_settings` (655 lines)
  and `xenon_store`. GPUI: `settings/`, `dropdown/` (about 3,200). GTK: `prefs*.rs` on
  `AdwPreferencesDialog` (about 1,560). Status: done. Markdown writing values are saved
  but not drawn until the Markdown branch merges.
- **Themes.** Ten families as Zed-schema JSON in `xenon_terminal/assets/` (11 files,
  about 290 KB). GTK reads the same files from there (`theme.rs`, 188 lines). Status:
  done, but the files live in a crate that is about to be deleted.
- **Keymap.** Defaults per platform in `xenon_keymap` (793 lines), user `keymap.json`.
  GTK: `keys.rs` (198) turns bindings into accelerators. Status: done.
- **Fonts.** Lilex and IBM Plex Sans bundled in `xenon_settings`. Status: done.

### Phone remote

- **Server, pairing, terminal streaming, uploads.** `xenon_remote` (3,700 lines,
  including the 1,780-line `page.html` phone app). Tailnet only. GTK host: `remote*.rs`
  (about 1,550). GPUI host: `xenon_ui/src/app/remote/` and
  `xenon_terminal/src/view/remote*.rs` (about 2,550). Status: done in GTK except tasks,
  mouse clicks, dim text, OSC 8 links, cursor hiding, and the phone's own light or dark
  choice.

### SSH

- **SSH workspaces.** Browse and edit remote text files over OpenSSH with a bundled
  Python helper, and reopen persistent tmux terminals. `xenon_ssh` (520 Rust lines plus
  `helper.py` 280), `xenon_finder` remote bits. GPUI UI:
  `app/ssh.rs`, `workspace_picker/{ssh,remote,host}.rs`,
  `xenon_editor/src/view/remote.rs` (about 1,090). GTK: not started. An SSH workspace
  shows an "unsupported" message (`dialogs.rs:24`). Design: `docs/ssh-workspaces.md`.

### IDE protocol for Claude Code

- **Local WebSocket MCP server** so Claude Code (and a Codex context socket) can open
  files and read the selection. `xenon_ide` (940 lines, GPUI-free) plus `PROTOCOL.md`.
  GPUI wiring: `app/services.rs` and a few call sites. GTK: not started.

### LSP

- **Language server client and host** (Rust, TypeScript). Go to definition (F12),
  diagnostics (F8), document highlight. `xenon_lsp` (990 lines, GPUI-free). GPUI UI:
  `app/lsp.rs` and `app/lsp/` (about 870) plus diagnostic drawing in the editor. GTK:
  settings page only; no client wiring. Design doc `lsp-navigation.md` (781 lines).

### Tasks

- **VS Code `tasks.json` support.** Parse, resolve variables and inputs, run each task
  in its own task tab, folderOpen consent keyed by hash. `xenon_core/src/tasks/` (1,520
  lines including 190 of tests), `xenon_store/src/task_trust.rs` (64). GPUI UI:
  `app/task*.rs`, `task_input.rs`, `task_picker.rs`,
  `xenon_terminal/src/view/task_run.rs` (about 1,930). GTK: not started; parsing is
  shared but unused by `xenon_gtk`.

### Worklist

- **Capture and list view for `.xenon/worklist.md`.** Quick capture (Ctrl+Shift+K), a
  list tab with sections, check-off, edit. GPUI: `worklist_capture.rs`,
  `app/worklist.rs`, `xenon_editor/src/{worklist_file,item_editor,view/worklist}` (about
  2,250). GTK: not started. The file format parser (`worklist_file/`, 966 lines) is
  GPUI-free.

### `xenon open` CLI and managed skill

- **`xenon open FILE[:LINE]`** hands a file to the running instance over a Unix socket.
  Parser and protocol in `xenon_store` (`cli.rs`, `ipc.rs`, `open_spec.rs`, about 440,
  GPUI-free). GPUI server: `crates/xenon/src/main.rs`, `app/open_cli.rs`,
  `app/open_ops.rs` (about 740). GTK: not started (worklist "Next"). Today a GTK
  terminal does not inherit `XENON_DATA_DIR`, so `xenon open` reaches the GPUI app.
- **Managed agent skill.** Installs the `xenon` skill under the agents' skill folders.
  `xenon_store/src/skill.rs` and `skill.md` (about 300). GPUI: first-run prompt
  `app/skill_prompt.rs` (257). GTK: Settings, Agents page only, no first-run prompt.

### Release and install

- **GPUI install (macOS bundle, codesign, Linux dev install, A/B slot launcher).**
  `scripts/release/{install,bundle,codesign.sh,sign_setup,install-linux}`,
  `scripts/xenon` (532 lines), `bin/install`, `macos/`,
  `crates/xenon_stub`.
- **GTK install.** `scripts/release/install-gtk` (63), `scripts/xenon-gtk` (39). Own app
  id `dev.xenon.Gtk`, own state dir `~/.xenon-gtk`, seeded from the GPUI app.
- **Gates.** `bin/check`, `scripts/health`, pre-commit hook, `scripts/check-*`. No CI
  directory exists (`.github` is absent); `scripts/health` is the manual full run.

### Diagnostics

- **Memory panel.** Footprint and per-terminal grid estimates, written to JSON.
  `xenon_memory` (133 lines), `xenon_ui/src/app/memory.rs` (211). GTK: none.
- **GPUI visual tests.** `crates/xenon/src/visual_test_runner*` (about 1,000),
  `xenon_ui/src/app/visual/` (about 1,800), `scripts/visual-tests`, `visual-review/` (2
  MB of HTML mocks and PNGs), `docs/visual-tests.md`. macOS Metal only.

### Smaller items found along the way

- **Toasts.** GPUI: `xenon_design_system/src/toast/` (about 550) and `app/toasts.rs`.
  GTK: `adw::Toast`, one helper. Done.
- **Embedded browser.** Design only (`docs/designs/embedded-browser.md`); no code.
  `PRODUCT.md` says a WebKit tab is "in scope" while `docs/project-state.md` says there
  is no webview. The two disagree.
- **Keep-awake while a phone is attached.** `xenon_remote/src/keep_awake.rs`. Done for
  both OSes; the macOS half (`caffeinate`) is dead on Linux.

## Cut candidates

These are proposals. The user decides. They are ranked by how little the user would miss
them against how much upkeep they remove: the first item is the best cut. Line counts
are the GPUI or shared lines that stop needing upkeep.

1. **macOS-only code.** Reason: macOS support is being dropped.
   - Saves: `crates/xenon_stub` (294 lines plus its test), `macos/` (2 MB with the icon
     source), `scripts/release/{bundle,codesign.sh,sign_setup,install}`,
     `scripts/ensure-metal`, `bin/install`, the macOS branches in
     `xenon_memory/src/lib.rs`, `xenon_remote/src/{keep_awake,names}.rs`, the
     `objc2-app-kit` dependency, the tracked `.DS_Store`, and the Homebrew and
     notarization items in `docs/release.md`. About 700 lines and 8 files of scripts.
   - Loses: nothing on Linux. The README and `PRODUCT.md` stop promising macOS.
2. **The custom Vim engine** (`xenon_editor/src/vim/`, 4,140 lines, plus `view/ex_io.rs`
   and the Vim notes in `docs/project-state.md`).
   - Reason: GtkSourceView ships `VimIMContext`, and the GTK editor already uses it.
   - Saves: 4.1k lines of the hardest-to-test code, and the "Neovim as oracle" test
     habit.
   - Loses: visual-block painting, `:s` with flags, blockwise registers, and other
     extras the custom engine added. GtkSourceView's Vim is a smaller subset.
3. **Tree-sitter highlighting and the fifty grammar crates** (`xenon_editor/src/
   highlight/`, `queries/`, 50 `tree-sitter-*` workspace dependencies,
   `docs/designs/syntax-highlighting-system.md`).
   - Reason: GtkSourceView highlights files itself. The GTK diff tab already does not
     use the shared `Highlighter` hook.
   - Saves: 780 lines, 50 compiled grammars (the largest share of clean build time and
     binary size after GPUI), and a 300-line design doc.
   - Loses: Markdown emphasis detail tuned in tree-sitter queries, and highlighting for
     languages GtkSourceView lacks (check Solidity, Zig, Elm, Svelte, Nix, PowerShell).
4. **VS Code `tasks.json` support** (about 3,500 lines across `xenon_core/src/tasks`,
   `xenon_ui` task files, `xenon_terminal` task_run, `xenon_store/task_trust.rs`, the
   phone remote's task list, and `docs/designs/vscode-tasks.md`).
   - Reason: agent work happens in terminals; a build is one `cargo` or `make` line in a
     terminal tab. The product model never mentions tasks.
   - Saves: about 3.5k lines, the folderOpen consent design, and a security surface.
   - Loses: Ctrl+Shift+B, the task picker, and tasks on the phone.
5. **The in-app worklist UI** (about 2,250 lines; keep the Markdown file).
   - Reason: `.xenon/worklist.md` is a plain Markdown file that agents and the editor
     already handle. The capture toast and list tab are extra chrome.
   - Saves: about 2.2k lines and the worklist parts of `docs/keyboard-first.md`.
   - Loses: Ctrl+Shift+K quick capture and check-off in a list view. The `xenon` skill
     reads and writes the file directly, so agent use is unaffected.
6. **LSP in the editor** (`xenon_lsp` 990 lines, GPUI wiring about 870, diagnostics
   drawing, `docs/designs/lsp-navigation.md` 781 lines, the Language Servers settings
   page).
   - Reason: language servers cost hundreds of MB, against the low-memory goal, and the
     editor is secondary to agents.
   - Saves: about 2k lines of code and a 781-line design doc, and the open "Rust
     Glancer" research item.
   - Loses: F12 go to definition, F8 diagnostics, document highlight. Worth a decision
     before the GTK "full editor" work starts, since that item is next on the worklist.
7. **SSH remote files** (Python helper, remote file tree and remote editor, about 1,090
   UI lines plus `xenon_ssh` 520 plus 440 Python). A lighter option: keep SSH as "open a
   terminal tab running `ssh host` in tmux" and drop remote file browsing and editing.
   - Saves: about 2k lines and the helper protocol.
   - Loses: browsing and editing remote files in the sidebar and editor. Ask whether it
     is used weekly; the homelab machines suggest it might be.
8. **Claude IDE bridge** (`xenon_ide` 940 lines plus `PROTOCOL.md`; at least the Codex
   socket, `codex_protocol.rs` 168 lines).
   - Reason: `PRODUCT.md` calls it "optional garnish", and the `xenon open` CLI covers
     opening files from any agent.
   - Saves: about 1k lines and the `tungstenite` duplication path.
   - Loses: Claude Code's `/ide` selection sharing and diff review in Xenon.
9. **Memory panel** (`xenon_memory` 133 lines, `app/memory.rs` 211 lines).
   - Reason: any system monitor shows memory.
   - Saves: about 350 lines. Loses: an in-app panel and a JSON snapshot for AI tools.
10. **Image viewer in the editor** (about 700 lines). GTK already sends unsupported
    files to the default app. Saves 700 lines. Loses: viewing images in a tab.
11. **A/B slot launcher** (`scripts/xenon` 532 lines,
    `XENON_SLOT` and `SLOT_DIRS` logic in `xenon_store`, `~/.xenon-shared`,
    `.xenon-active-slot`).
    - Reason: GTK already runs side by side with `XENON_GTK_APP_ID` and
      `XENON_GTK_DATA_DIR`. After cutover there is one app.
    - Saves: about 700 lines and the "copy state at flip time" design. Loses: the
      one-command dogfood flip while developing Xenon inside Xenon (an env var replaces
      it).
12. **Trim the theme pack** from ten families to the few actually used. Each JSON is
    about 27 KB of mostly Zed boilerplate. Saves a few hundred KB and a test matrix.
    Loses: the dropped themes. Low value either way.
13. **Back and forward history** (Ctrl+[ and Ctrl+], about 300 lines). Rarely a daily
    flow for an agent shell. Loses: browser-style jump back.

Not proposed for cutting, but worth watching: the phone remote (3.7k shared lines plus
1.5k GTK) is large for one feature, though it is the stated differentiator and is
already ported. Its lowest-value parts are the Low bandwidth toggle and the keep-awake
helper.

## Dead code and simplifications

Every item below was confirmed by `rg -w` for callers across `crates/`, or by reading
the manifest. `unused = deny` is on for every crate, so private dead code cannot exist;
these are `pub` items, manifests, and structure that the compiler does not flag. I did
not compile anything.

### No callers (confirmed with `rg -w`)

- `xenon_core::SessionState::clamp_widths`, `crates/xenon_core/src/session.rs:40`. No
  call site anywhere.
- `xenon_core` task `is_ready`, `crates/xenon_core/src/tasks/background.rs:54`. Defined
  only. (`feed` sets the flag; nothing reads it back.)
- `crates/xenon_design_system/src/multiline_text/movement.rs:65` `split_at_caret`.
- `crates/xenon_design_system/src/text_input/config.rs:66` `submit_on_plain_enter`.
- `crates/xenon_terminal/src/view/mod.rs:336` `toggle_mouse_to_app`.
- `crates/xenon_terminal/src/view/mod.rs:626` `viewport_text`.
- `crates/xenon_editor/src/view/worklist.rs:103` `visual_worklist_add_to`.
- `crates/xenon_terminal/src/init.rs:102` `theme_names` and
  `crates/xenon_design_system/src/settings/prose.rs:23` `prose_font` are re-exported
  (`xenon_terminal/src/lib.rs:36`, `settings/mod.rs:21`) and never called.
- `xenon_keymap::to_json` (`defaults.rs:200`), `set_display_overrides`
  (`display.rs:16`), and `STARTER_KEYMAP` are called only by `xenon_ui/src/keymap.rs`.
  They die with GPUI.
- Public but only used inside their own crate (narrow to `pub(crate)` or inline):
  `xenon_git::load::all_added`, `xenon_git::status::GitFile::is_conflict`,
  `xenon_remote::phone_base_url` (re-exported at `lib.rs:47`, no caller),
  `xenon_remote::host::...::CAPACITY`, `xenon_store::WindowGeometry::MIN_WIDTH` and
  `MIN_HEIGHT`, `xenon_core::Registry::workspace_for_path`.

### Unused manifest entries

- `crates/xenon_terminal/Cargo.toml:23` `collections`: no `collections::` use in the
  crate.
- `crates/xenon_ui/Cargo.toml:48` `libc`: no use in the crate.
- Every `[features]` block is `visual-tests` (`xenon`, `xenon_ui`, `xenon_terminal`,
  `xenon_editor`, `xenon_design_system`), used only by the GPUI visual test runner (52
  `cfg(feature = "visual-tests")` sites). `xenon`'s optional `anyhow`, `serde`,
  `serde_json` exist only for that feature. All go with GPUI.
- All other workspace dependencies are referenced by some crate (checked name by name
  against each `Cargo.toml`), and every `tree-sitter-*` grammar is referenced in
  `highlight/grammars.rs`.
- `xenon_gtk` pulls `notify` (Zed's fork) on the git branch for the git watcher, while
  `files.rs` already uses `gio::FileMonitor`. Use one mechanism (GIO) and drop the fork
  plus its `[patch.crates-io]` pin.
- `xenon_gtk` depends on `nucleo` directly (`picker.rs:15`) as well as through
  `xenon_finder`. Export one ranking helper from `xenon_finder` and drop the direct
  dependency.

### Logic copied in both frontends

- **Terminal activity rule.** `xenon_core/src/activity.rs` (`TerminalActivity`) is used
  by GTK. `xenon_terminal/src/view/mod.rs` (fields at 202 to 210, `wakeups` increment at
  376, quiet timer at 450 to 470) still carries its own copy. Resolved by deleting GPUI;
  do not spend time unifying.
- **Workspace picker ranking.** `xenon_gtk/src/picker.rs` re-implements the GPUI rules
  in `xenon_ui/src/workspace_picker/{candidate,rank}.rs` (about 300 lines each side),
  and its header says so ("The rules are the GPUI picker's"). Keep the GTK one, or move
  the ranking into `xenon_finder` first if GPUI will live a while longer.
- **Git-dirt watcher.** `xenon_ui/src/app/git_dirt.rs` (376) and the branch's
  `xenon_gtk/src/git.rs` (282) both build a debounced watcher plus worker. The pure path
  rules are already shared in `xenon_git`. The GPUI half deletes.
- **keymap.json parsing.** `xenon_gtk/src/keys.rs:125` (`user_entries`) and
  `xenon_ui/src/keymap.rs` (`entries_of`) both turn the user's file into
  `xenon_keymap::Entry`. Move the parse into `xenon_keymap` so there is one.
- **Markdown block model.** GPUI `xenon_editor/src/prose/model/` (pulldown to blocks,
  about 1,300 lines) and GTK `xenon_prose/src/parse.rs` (382, unmerged) both parse
  Markdown into styled blocks. The new crate says so in its header. Resolved by deleting
  the GPUI one.
- **Theme loading.** GPUI reads the JSON through Zed's `theme_settings`
  (`xenon_terminal/src/init.rs`); GTK parses the same files in `theme.rs`. Resolved by
  deletion; the JSON files must move first.
- **Phone remote host glue.** `xenon_ui/src/app/remote/` and `xenon_gtk/remote*.rs` are
  two adapters over `xenon_remote`. Already thin; resolved by deletion.

### Leftover shims from the migration

- `xenon_gtk/src/data.rs` (98 lines): seeds `~/.xenon-gtk` from the GPUI slot and
  handles `--reseed`. Delete at cutover, with `docs/project-state.md` paragraph "The GTK
  frontend ... keeps its own state directory".
- `xenon_gtk/src/dialogs.rs:24` `show_unsupported`: the placeholder for SSH.
- `xenon_gtk/src/keys.rs:7` `ACTIONS`: maps GPUI action names (`xenon::NewTerminal`,
  `xenon_terminal::Find`) to GTK actions. The keymap vocabulary and users' `keymap.json`
  files still speak GPUI action names. Rename once, then drop the table.
- `xenon_keymap` writes defaults as macOS glyphs and converts to Linux
  (`defaults.rs:162`, `effective.rs:49`, `chord.rs`). With macOS gone, author the Linux
  bindings directly and delete the converter and `Platform::Mac`.
- `xenon_settings/src/families.rs:18` `.ZedMono` and `Zed Plex` aliases, and
  `xenon_ui/src/dropdown/fonts.rs:14`: Zed font names that only GPUI advertises.
- `xenon_store`: `SLOT_DIRS`, `shared_dir()`, `remote_devices.rs:36` slot detection,
  `ipc.rs` comments about A/B slots. The device book can live in the one data dir.
- `docs/designs/mobile-pty-remote.md` (252 lines): already marked superseded by v2.
  `docs/designs/workspace-queue-exploration.md` (9 lines): "superseded" stub.
  `HYGIENE-REMAINING.md`: about GPUI's `XenonApp` peels. `.cursor/rules`: empty
  directory. `tmp/`: ignored but present.
- `docs/project-state.md` is mostly GPUI and macOS gotchas (about 60% by line count).
  After cutover it should be rewritten, not trimmed.

### Over-general abstractions with one user (after GPUI goes)

- `PaneContent<T>`, `PaneLeaf<T>`, `PaneTree<T>` in `xenon_core` are generic over the
  frontend's tab type. GTK will be the only user (`Tab`). Keep the generics (they make
  the tests simple) or make them concrete; low priority.
- `xenon_git::diff::Highlighter` (`diff.rs:9`) is a function-pointer hook so a frontend
  can supply syntax spans. Only GPUI supplies one; GTK lets GtkSourceView color it. Drop
  the parameter from `parse_unified`, `load`, and `all_added` after cutover.
- `xenon_settings::FontProbe` (`families.rs:7`) exists so GPUI and Pango can each probe
  fonts. One implementer will remain.
- `xenon_remote/src/frame.rs` models a cell grid for the GPUI path; GTK fills the same
  `Screen` from VTE HTML (`vte_html.rs`). Still needed as the wire model, but check
  which `Style` fields only the GPUI snapshot ever set.
- `xenon_keymap::display_*` and the override map exist to print shortcut text in GPUI
  menus; GTK shows accelerator labels itself.

### Scripts no longer needed after cutover

`scripts/visual-tests`, `scripts/check-ui-boundaries` (scans `xenon_ui`, `xenon_editor`,
`xenon_terminal`), `scripts/check-zed-pins`, `scripts/ensure-metal`, `scripts/xenon`
(slot launcher),
`scripts/release/{bundle,codesign.sh,sign_setup,install,install-linux}`, `bin/install`.
Also drop the matching steps from `bin/check`, `scripts/health`,
`scripts/git-hooks/pre-commit`, and `.vscode/tasks.json` (`xenon` and `install` tasks).
`scripts/check-rust-shape` and its baseline TSV list GPUI files and shrink.

## What becomes deletable when GPUI goes

**Crates (about 56k own lines):** `xenon` (binary and visual test runner), `xenon_ui`,
`xenon_editor`, `xenon_terminal`, `xenon_design_system`, `xenon_stub`.

**Vendored code (about 22k lines, 1 MB):** `third_party/gpui_linux` and
`third_party/terminal`, with their README and LICENSE files, the `[patch.'https://
github.com/zed-industries/zed']` block, and the `exclude` line in `Cargo.toml`.

**Workspace dependencies:** all ten Zed git dependencies (`gpui`, `gpui_platform`,
`terminal`, `theme`, `theme_settings`, `settings`, `release_channel`, `util`, `task`,
`collections`); `semver`; the 50 `tree-sitter*` entries; `ropey` (check `xenon_prose`
first); `lucide-icons`; `trash` (GIO can trash files); `image`; `unicode-segmentation`;
`objc2-app-kit`; `env_logger`; the `[patch.crates-io]` entries for `async-process`,
`async-task`, and the `notify` forks (if GTK moves its git watcher to GIO); `lsp-types`
and `url` only if LSP is cut. `ignore`, `nucleo`, `ropey`, `pulldown-cmark`, `qrcode`,
`tungstenite`, `mio` stay while their users stay.

**Scripts:** see the list above.

**Docs and assets:** `DESIGN.md` sections about GPUI chrome (keep the visual language),
`docs/visual-tests.md`, `docs/keyboard-first.md` parts about macOS keys,
`docs/designs/pane-tree-layout.md` (714 lines, GPUI implementation notes),
`docs/designs/syntax-highlighting-system.md`, `docs/designs/mobile-pty-remote.md`,
`docs/designs/workspace-queue-exploration.md`, `HYGIENE-REMAINING.md`, `visual-review/`
(2 MB; the HTML mocks are GPUI-era design evidence, decide whether to keep any), the
`macos/` directory, `ATTRIBUTION.md` rows for Zed terminal files (keep rows for anything
that survives, for example the theme JSON).

**CI pieces:** there is no CI directory. The gates that go are `ui-boundaries`,
`zed-pins`, `metal`, and the GPUI visual tests inside `scripts/health`. The `lint` and
`test` steps there will run much faster without the 946-crate tree.

## What must move first because GTK still needs it

In order of how much blocks cutover:

1. **`xenon open` socket server in GTK.** The CLI parser and protocol are already in
   `xenon_store` (GPUI-free). GTK needs the server and to inject `XENON_DATA_DIR` into
   its shells. Until then every agent's `xenon open` reaches the wrong app. Small.
2. **Theme JSON files.** `xenon_terminal/assets/*.json` (11 files) are read by GTK
   `theme.rs`. Move them to `xenon_settings` (which already holds the theme lineup and
   fonts) before deleting `xenon_terminal`. Update `ATTRIBUTION.md` paths.
3. **State directory.** Make `~/.xenon` the GTK default and delete `data.rs` seeding.
   Move `remote_devices` out of the shared-dir logic.
4. **Copy Clean.** `xenon_terminal/src/clipboard.rs` (`clean_agent_output`) and
   `bar_gutter.rs` are pure text logic (about 440 lines with the GPUI clipboard glue
   removed). Move the text functions into a GPUI-free crate (`xenon_core` or a small new
   one) and wire Ctrl+Shift+C in GTK.
5. **Editor pieces, only if kept:**
   - `markdown/link.rs` (324 lines, GPUI-free): link and path-under-cursor follow.
   - `worklist_file/` parser (966 lines, GPUI-free) if the worklist UI stays.
   - GPUI-free and not needed: `vim/` (GtkSourceView has its own), `highlight/`.
   - The editor `buffer/` rope (650 lines) is not needed; GtkTextBuffer replaces it.
6. **LSP, IDE bridge, SSH, tasks:** the crates `xenon_lsp`, `xenon_ide`, `xenon_ssh`,
   and `xenon_core::tasks` are already GPUI-free. What is missing is GTK wiring, which
   is new work, not a port. Each is either a cut candidate above or a "Next" item.
7. **Keymap parse and action vocabulary** (see the shim list): move `user_entries` into
   `xenon_keymap` and settle the action names.
8. **Git branch merge.** `xenon_git` needs the `watch.rs` additions already on the
   branch, and `xenon_gtk` must stop depending on the `Highlighter` hook.

## Recommended GPUI deletion order

1. Merge the two worktree branches (git section and diff tab; Markdown prose) so the
   deletion does not collide with them. The git branch edits `xenon_ui` files.
2. Decide the cut list above, so nothing gets ported just to be deleted. Items 2 to 6
   change what has to move.
3. Do the "move first" list: `xenon open` server, theme JSON, Copy Clean, keymap parse,
   state directory default.
4. Cut over: install the GTK build as `xenon`, run it on `~/.xenon`, and live on it for
   a short period with GPUI still in the tree but not installed.
5. Delete in one commit, outermost first so the build tells you what is left:
   `crates/xenon` and `scripts/visual-tests`, then `xenon_ui`, `xenon_editor`,
   `xenon_terminal`, `xenon_design_system`, `xenon_stub`, then `third_party/`, then the
   workspace dependencies and patch blocks in `Cargo.toml`, then scripts and gates.
6. Follow with the small cleanups in the dead-code list (unused `pub` items,
   `Highlighter` hook, `Platform::Mac`, slot logic, Zed font aliases).
7. Rewrite `README.md`, `PRODUCT.md`, `AGENTS.md`, `DESIGN.md`, and
   `docs/project-state.md` for Linux and GTK last, when the code answer is final.

Do the build work for step 5 in one place. The 14 GB machine ran out of memory last
night; removing the Zed tree is the single largest cut to build memory.
