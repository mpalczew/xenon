# GTK library review: what could we delete?

Written 2026-11-07 from `main` plus the two unmerged branches. Nothing here is a
decision. It is a research pass for the user to accept or reject, feature by feature.

The question for every feature: could a GTK, libadwaita, VTE or GtkSourceView built-in,
a desktop service, or a well-maintained crate replace our code and cut it significantly?
The direction is "prefer GTK built-ins; the less code the better", with low CPU and
memory, very low latency, and keyboard-first use as hard limits. A built-in that looks a
little different from the old GPUI app is fine. Text the user sees stays word for word.

## How to read the numbers

- "Current" lines are `wc -l` over the Rust files named in the entry (tests included).
  Where a file lives only on a branch the entry says so. "A" means
  `worktree-agent-a6101b1c2f9016601` (Git section, diff tab, toolbar); "B" means
  `worktree-agent-ad3978011b03424c7` (Markdown, `xenon_prose`).
- "After" and "saved" are my estimates, rounded. They count Rust only. I compiled
  nothing and ran no GTK code, so every estimate is a reading of the APIs, not a
  measurement. Anything I could not confirm from documentation or headers is marked
  "unverified" and should get a short spike before anyone commits to it.
- GPUI-only code is ignored except to understand behavior, because it is being deleted.
  Features that GTK has not drawn yet are listed separately, in "Not yet in GTK", with
  the way they should be built.

## What is installed on this machine

| Library         | System version               | Rust binding in `Cargo.lock` / on crates.io      |
| --------------- | ---------------------------- | ------------------------------------------------ |
| GTK 4           | 4.22.4                       | `gtk4` 0.11.5                                    |
| libadwaita      | 1.9.1                        | `libadwaita` 0.9.2 (we enable feature `v1_9`)    |
| VTE (GTK 4)     | 0.84.0                       | `vte4` 0.10.0                                    |
| GtkSourceView 5 | 5.18.0                       | `sourceview5` 0.11.2                             |
| GLib / GIO      | 2.88.0                       | `glib`, `gio` 0.22.10                            |
| Pango           | 1.57.0                       | via `gtk4`                                       |
| libsoup 3       | 3.6.6, runtime only          | `soup3` 0.9.0 (needs `libsoup-3.0-dev`, missing) |
| libspelling     | not installed, apt has 0.4.9 | `libspelling` 0.5.0 (24k downloads, young)       |
| WebKitGTK 6     | 2.52.6, runtime only         | `webkit6` 0.6.1 (needs the dev package, missing) |
| libgit2         | 1.9.1, runtime only          | `git2` 0.21.0                                    |
| git             | 2.53.0                       | called as a command today                        |
| GVfs            | not installed (no `gvfs*`)   | none needed, GIO loads it at run time            |
| libportal       | 0.9.1                        | `ashpd` 0.13.12 is already in the lock file      |

Other crate versions checked on crates.io: `similar` 3.2.1, `imara-diff` 0.2.0, `gix`
0.90.0, `notify` 8.2.1 (we pin a 9.0.0-rc.4 fork), `nucleo` 0.5.0, `nucleo-matcher`
0.3.1, `fuzzy-matcher` 0.3.7, `ignore` 0.4.34, `clap` 4.6.7, `tungstenite` 0.30.0 (we
lock 0.24.0), `russh` 0.65.1, `ssh2` 0.9.7, `qrcode` 0.14.1, `pulldown-cmark` 0.13.6 (we
lock 0.12.2), `trash` 5.3.0.

Two machine facts matter. The language and style files of GtkSourceView 5.18 are
compiled into the library, so I could not list them from disk; checking whether it knows
Solidity, Zig, Elm, Svelte, Nix or PowerShell needs one call to
`sourceview::LanguageManager::default().language_ids()`. And GVfs is absent here, which
decides the SSH entry.

---

# Summary

## 1. All entries, sorted by lines saved

"Saved" is Rust lines that stop needing upkeep. Entries marked * depend on a spike or a
user decision named in their text.

| #  | Feature                                   | Current | After | Saved | Verdict                   |
| -- | ----------------------------------------- | ------: | ----: | ----: | ------------------------- |
| 1  | Theming and the theme picker              |   1,040 |   200 |  ~840 | replace (look changes) *  |
| 2  | SSH workspaces                            |     965 |   220 |  ~745 | replace if GVfs is OK *   |
| 3  | Keymap and shortcut text                  |     991 |   350 |  ~640 | partial                   |
| 4  | Diff view                                 |   1,330 |   750 |  ~580 | partial *                 |
| 5  | Phone remote HTTP and WebSocket transport |     894 |   350 |  ~540 | partial *                 |
| 6  | Files tree                                |     551 |   220 |  ~330 | replace                   |
| 7  | Tab strip                                 |     470 |   170 |  ~300 | replace (already decided) |
| 8  | `xenon open` socket and CLI parsing       |     437 |   140 |  ~300 | replace                   |
| 9  | Settings pages and setters (minus themes) |   1,873 | 1,700 |  ~170 | partial                   |
| 10 | Git status watcher                        |     323 |   150 |  ~170 | partial                   |
| 11 | Ctrl+P finder and ranking                 |     628 |   470 |  ~160 | partial                   |
| 12 | Claude IDE WebSocket server               |     918 |   770 |  ~150 | partial, or cut *         |
| 13 | Memory diagnostics                        |     133 |     0 |  ~130 | delete                    |
| 14 | Fonts (bundled files and family probing)  |     130 |    20 |  ~110 | replace                   |
| 15 | Cutover leftovers (`data.rs`, slot logic) |     180 |     0 |  ~180 | delete (not a library)    |
| 16 | Palette shell                             |     469 |   390 |   ~80 | partial                   |
| 17 | Empty state                               |     132 |    50 |   ~80 | replace                   |
| 18 | Terminal find bar                         |     135 |    75 |   ~60 | partial                   |
| 19 | Workspace list in the sidebar             |     291 |   230 |   ~60 | partial                   |
| 20 | Dialogs                                   |     130 |    80 |   ~50 | partial                   |
| 21 | Terminal activity, attention, pulse       |     632 |   560 |   ~70 | keep (drop the pulse)     |
| 22 | Toolbar and custom icons                  |      40 |    10 |   ~30 | replace                   |
| 23 | Pane tree and splits                      |   1,342 | 1,300 |   ~40 | keep                      |
| 24 | Editor (plain files)                      |     181 |   210 |   -30 | keep, gains features      |
| 25 | Markdown prose mode (branch B)            |   1,900 | 1,900 |     0 | keep                      |
| 26 | Terminal links                            |      88 |    80 |   ~10 | keep                      |
| 27 | LSP                                       |     993 |   993 |     0 | keep (or cut)             |
| 28 | Phone remote screen mirroring             |    ~960 |  ~960 |     0 | keep                      |
| 29 | VS Code `tasks.json`                      |   1,588 | 1,588 |     0 | none exists; product call |
| 30 | Session save and restore, state storage   |    ~600 |  ~520 |   ~80 | keep                      |
| 31 | Toasts, Vim, syntax highlighting          |       - |     - |     0 | done by the toolkit       |
| 32 | CLI argument parsing (`clap`)             |      20 |    20 |     0 | keep, do not add `clap`   |
| 33 | Test and diagnostics hooks                |    ~500 |  ~500 |     0 | keep                      |

Adding it up, table rows 1 to 22 save about 5,800 lines. The next table is work that GTK
has not drawn yet, where the right building blocks avoid a much bigger port.

### Not yet in GTK: the lines these avoid

The GPUI size is what a port would have cost; the estimate is what to write on GTK.

| Feature                        |  GPUI size | Build on GTK from                                            | Estimate |
| ------------------------------ | ---------: | ------------------------------------------------------------ | -------: |
| Worklist capture and list      |      2,250 | Markdown file in a normal editor tab, `adw::Dialog` to add   |       60 |
| Image viewer                   |        700 | `gtk::Picture::set_file`                                     |       40 |
| Command palette                |        480 | the existing palette shell plus the `app.*` action list      |      100 |
| Drag-to-split                  |       ~400 | `gtk::DropTarget` on the pane body plus `adw::TabView`       |      120 |
| Dirty-close prompt             |        156 | `adw::AlertDialog` in `TabView::connect_close_page`          |       25 |
| Tab context menu, tab overflow | (in 1,600) | `TabView::set_menu_model`, `TabBar` scrolling                |       25 |
| LSP diagnostics, hover, go-to  |     ~1,100 | GtkSourceView marks, `Hover`, `Completion` providers         |      350 |
| Files context menu             | (in 1,300) | `gtk::PopoverMenu`, `gtk::FileLauncher`, `File::trash_async` |       80 |

## 2. Top 10 by lines saved per unit of risk

Ranked by how much Rust goes away against how likely it is to hurt. "Risk" is my
judgment: low means a stock widget I am confident covers the behavior.

1. **Worklist as a Markdown tab with a small capture dialog.** Avoids about 2,200 lines;
   risk low, since the file is plain Markdown and the editor already exists.
2. **Delete the bundled fonts.** About 110 lines and 1.6 MB of TTF files that the GTK
   app never registers. Risk very low.
3. **Delete `xenon_memory`.** 133 lines, no replacement needed. Risk none.
4. **Native accelerators instead of `xenon_keymap`'s conversion layers.** About 640
   lines. Risk low: `keys.rs` already produces GTK accelerators.
5. **Files tree on `gtk::DirectoryList` and `gtk::TreeListModel`.** About 330 lines and
   the context menu comes cheaper. Risk low to medium: one spike on keyboard behavior.
6. **Tab strip on `adw::TabView` and `adw::TabBar`.** About 300 lines; already decided.
   Risk low, with one spike on the drag-to-split drop.
7. **`xenon open` through `GApplication`.** About 300 lines and a free
   start-if-not-running path. Risk low to medium: D-Bus must exist where agents run.
8. **Themes from libadwaita and GtkSourceView schemes.** About 840 lines and 288 KB of
   JSON. Technical risk low; the user-visible change is large, so it needs a yes.
9. **Git watcher on `gio::FileMonitor` and `gio::Subprocess`.** About 170 lines and the
   `notify` fork pin. Risk low; the dirt tick updates a little later during an edit
   burst.
10. **Diff view on shared `GtkAdjustment`s and `GtkSourceMark` categories (option A).**
    About 580 lines. Risk medium: needs a GObject subclass for the gutter and a spike
    for scroll sync.

Close behind: SSH over GVfs (745 lines, but GVfs is not installed here), phone remote
transport on libsoup (540 lines, new system package), and the finder on `nucleo`'s own
worker (160 lines).

## 3. Where a library adds more risk than it saves: keep our code

- **`git2` or `gix` for git status.** The command line is the fastest correct answer for
  a status of a large working tree (it has the untracked cache), is already written, and
  costs no build. `gix` 0.90 pulls in hundreds of crates. `git2` links libgit2, which is
  installed only as a runtime library.
- **`russh` or `ssh2` for SSH.** Both lose `~/.ssh/config`, `ProxyJump`, agent and
  `ControlMaster` behavior that the OpenSSH command line gives for free. `russh` also
  brings `tokio`.
- **`clap`.** We parse two options and one subcommand. `clap` adds about a dozen crates
  and compile time to save about 20 lines.
- **GSettings and dconf for settings.** It needs a compiled schema installed
  system-wide, loses the human- and agent-editable `settings.json`, and breaks the data
  directory model. `xenon_store` stays.
- **WebKitGTK for Markdown rendering.** A WebProcess costs on the order of a hundred
  megabytes and starts slowly, and a rendered view cannot be edited in place the way
  prose mode edits the one buffer. The dev package is not installed either.
- **xterm.js for the phone.** It needs the raw byte stream, and VTE gives no hook to tee
  its pty. Replacing 1,783 lines of `page.html` would mean owning the pty and feeding
  VTE ourselves, and shipping a 300 KB third-party script.
- **`gtk::FileDialog` as the workspace picker.** It is mouse-first and goes through a
  portal. The Ctrl+Shift+O picker (fuzzy over known workspaces, path completion) is the
  keyboard-first path and stays.
- **`gtk::ListView` for the workspace list.** Under fifty rows. A list model needs a
  GObject item subclass in Rust, which is more code than the `ListBox` it replaces.
- **`adw::OverlaySplitView` for the sidebar.** Its sidebar width cannot be dragged. We
  would give up the draggable divider and its saved width. Fine if the user does not
  mind, a loss otherwise.
- **`libspelling`.** Not installed, the crate is young, and it replaces nothing; it
  would only add spell check. Worth a later spike, not a dependency today.
- **`git status` replaced by GtkSourceView's `diff` language.** It is not a status tool;
  see the diff entry for where it does fit.

## 4. Shared crates that could be deleted or merged

- **Delete `xenon_memory`** (133 lines). Nothing in GTK calls it.
- **Delete `xenon_stub`** (175 lines, macOS dock stub). Already on the cut list.
- **Merge `xenon_settings` into `xenon_store`.** `xenon_settings` depends on
  `xenon_store` and nothing else uses it once GPUI is gone. Together they are about
  2,400 lines of "what the user chose and where it is saved". One crate removes a
  manifest, a lint block and a re-export layer.
- **Merge `xenon_keymap` into `xenon_gtk`** (a `keymap` module of about 350 lines) after
  the macOS glyph converter and the display override map die. Its only other consumer is
  `xenon_design_system`, which is deleted with GPUI.
- **Move `xenon_ssh`'s type out of `xenon_core`.** `xenon_core` depends on `xenon_ssh`
  only for `SshWorkspace`. If SSH moves to GVfs, the crate shrinks to `ssh_config.rs`
  and the tmux helper, and can become a module of `xenon_core`.
- **`xenon_ide` and `xenon_lsp`** are not wired into GTK. They are independent crates
  that either get wired or get deleted; the cut-list decision comes first. If the IDE
  bridge stays, share one WebSocket transport with `xenon_remote` and drop `tungstenite`
  and `mio` from both.
- **Keep as crates:** `xenon_core`, `xenon_finder`, `xenon_git`, `xenon_remote`,
  `xenon_prose`. They hold pure rules with no widget code, which is what the "views
  draw, models decide" rule wants, and none is small enough to be worth folding in.
  `xenon_prose` (694 lines on branch B, one consumer) is the closest call.

---

# Entries

Each entry: what it does and where; the replacement; what we delete and write; what
changes for the user; risks; verdict.

## 1. Pane tree and splits

**What it does.** Nested splits with fractions, tabs per pane, unsplit on last close,
parked empty panes, depth limits. Current code: `xenon_core` `pane_*.rs`, `content.rs`,
`split.rs` (1,198 lines) and `xenon_gtk/src/layout.rs` (144), which turns the tree into
a right-leaning chain of `gtk::Paned` and keeps divider fractions in step.

**Replacement:** `gtk::Paned` is already what we use. Nothing else in GTK 4.22 or
libadwaita 1.9 nests resizable splits. `adw::OverlaySplitView` and
`adw::NavigationSplitView` are two-pane sidebar layouts. `adw::MultiLayoutView` (1.6)
swaps whole layouts, it does not split. `adw::TabView` holds pages, not splits.

**Delete:** about 40 lines. The `place` tick callback that sets a divider from a
fraction once the widget has a size could use `Paned::set_position` after `map`, but
only if the first size is known, so I would leave it. **Still write:** all of it. The
tree model is the saved layout format and the rules are tested.

**User sees:** nothing changes.

**Risks:** none new. This is the one area where our model must stay, because
`AdwTabView` would otherwise become the second source of truth for tab order (see the
tab entry for how to keep them in step).

**Verdict: keep.** No GTK widget is a tree of resizable splits.

## 2. Tab strip

**What it does.** Per-pane tab chips with a status rail, close button, middle-click
close, and scrolling to the selected chip. Current: `pane.rs` 281 lines (+69 on A), the
pulse code in `events.rs` and `sidebar.rs` (about 70), and chip CSS in `theme.rs` (about
50). Total about 470.

**Replacement:** `adw::TabView` (one per pane) with `adw::TabBar`. Available since
libadwaita 1.0; we have 1.9.1 and binding 0.9.2.

- Page state maps one to one: `TabPage::set_needs_attention(bool)`,
  `set_indicator_icon(Some(&gio::Icon))`, `set_loading(bool)`, `set_title`,
  `set_tooltip`, `set_icon`.
- `TabView::connect_close_page` returns a `Propagation`; call
  `TabView::close_page_finish(page, confirm)` after an `adw::AlertDialog`. That is the
  dirty-close prompt.
- `TabView::connect_setup_menu` plus `TabView::set_menu_model(&gio::Menu)` gives the tab
  context menu (close others, move to new pane).
- `TabBar::set_autohide(true)` replaces the "lone tab in a lone pane needs no strip"
  logic. `set_expand_tabs(false)`, `set_start_action_widget` / `set_end_action_widget`
  hold the "+" button. Tab overflow scrolling is built in (our strip has none today).
- Reorder: `TabView::reorder_page` and built-in drag. Move between panes:
  `TabView::transfer_page`, which also re-parents the page child, which is what
  `PaneView::sync` does by hand.
- `TabView::connect_create_window` fires when a tab is dragged out of every bar; return
  `None` to refuse (Xenon is one window).

**Delete:** `make_chip`, `reveal`, `sync_chips`, `paint`, the chip CSS and the per-tab
pulse: about 300 lines. **Still write:** about 170: creating the views, mapping `Tab`
state to page properties, `close-page` and `setup-menu` handlers, and calls that keep
`PaneLeaf` in step on reorder and transfer. The model stays authoritative; the view
reports moves into it.

**User sees:** libadwaita's tab look (rounded, close button on hover or selected,
overflow arrows). The coloured status rail under each chip becomes an indicator icon or
the attention dot on the page, and the working pulse becomes a static mark or the
`loading` spinner. Tab overflow and the context menu are new.

**Risks:**

- `loading` spins at the display's frame rate, which costs CPU while an agent works. Use
  a static `indicator-icon` instead, and let the working state be a fixed dot. This is
  the one place where the stock widget tempts us into continuous animation.
- `AdwTabView` has its own shortcuts (Ctrl+Tab, Ctrl+PageUp/Down, Alt+1 to 9). They
  could shadow terminal keys. `TabView::set_shortcuts` can switch them off, or on
  selectively. Unverified which of these reach the terminal first.
- Each page child in a hidden tab stays alive, as with our `gtk::Stack`. No new cost.

**Verdict: replace.** Already decided; this sizes it at about 300 lines saved.

## 3. Drag-to-split

**What it does.** Drag a tab to another pane's centre to move it, or to a pane edge to
split. Current GTK code: none (the GPUI version, `app/tab_drop.rs`, is a few hundred
lines).

**Replacement:** `adw::TabBar` drags between bars for free (centre moves). For the edge
zones, put a `gtk::DropTarget` on the pane body's `gtk::Overlay` for the GType
`adw::TabPage`, compute the hovered edge in `connect_motion`, draw a translucent
rectangle (one CSS-styled `gtk::Box` child of the overlay), and on `drop` call the
existing `PaneContent` split and `TabView::transfer_page`.

**Delete:** nothing today; it avoids a port of a few hundred lines. **Still write:**
about 120 lines.

**User sees:** drag to split works from the tab strip, the way the GPUI app did.

**Risks:** unverified whether `AdwTabBar` offers the dragged page to a plain
`DropTarget` of type `adw::TabPage`. libadwaita's own bars accept it, and the content
provider carries the page object, so I expect yes, but spike it first. If it fails the
fallback is `TabBar::setup_extra_drop_target`, which is meant for foreign content and
would need a small shim.

**Verdict: replace** (build it on the built-ins rather than port).

## 4. Workspace list in the sidebar

**What it does.** Rows of workspace name, path and status dot, with select-to-activate.
Current: `sidebar.rs` 197 and `workspaces.rs` 94 lines, plus `model.rs` bookkeeping.

**Replacement:** keep `gtk::ListBox`, but build each row as an `adw::ActionRow` (title =
name, subtitle = path, `add_prefix` = the status dot) instead of boxes and labels.
`ListBox::bind_model` over a `gio::ListStore` would remove the manual rebuild, but needs
a GObject item type. `gtk::ListView` is the "right" widget for long lists; at under
fifty rows its factory and `GObject` subclass cost more code than they remove.
`adw::OverlaySplitView` / `NavigationSplitView` give a collapsible sidebar with
`show-sidebar`, `collapsed`, min and max width; they have no drag handle.

**Delete:** about 60 lines of row construction. **Still write:** the dot, rename,
reorder (drag and drop on `ListBox` has no built-in reorder; about 60 lines when it
comes), and the Files section below it.

**User sees:** slightly different row spacing. If we also adopt `OverlaySplitView`, the
sidebar width is no longer draggable.

**Risks:** `ActionRow` ellipsizes the subtitle at the end, but a path reads better
ellipsized at the start (the current code does this). Minor. Unverified whether
`subtitle-lines` plus a custom label would fix it; if not, keep the label.

**Verdict: partial.** Use `ActionRow`; keep the `ListBox` and the `Paned`.

## 5. Files tree

**What it does.** Lists the workspace folder, expands lazily, watches at most 48 open
folders, opens files, marks the open file. Current: `files.rs` 448 and `files_rows.rs`
103 lines. It is a flat `ListBox` that is rebuilt from a `FilesModel` on every change.

**Replacement:** `gtk::ListView` over a `gtk::TreeListModel`, with a `gtk::TreeExpander`
in each row.

- The root model is a `gtk::DirectoryList` (`DirectoryList::new(Some("standard::*"),
  Some(&dir_file))`). It enumerates asynchronously and, with `monitored` on, watches the
  directory with `gio::FileMonitor` by itself.
- `TreeListModel::new(root, passthrough=false, autoexpand=false, create_func)` where
  `create_func` returns a `DirectoryList` for a directory row. A child list exists only
  while its row is expanded, so the number of watchers equals the number of open
  folders, which is the bound our 48-folder cap enforces by hand.
- Order and filtering: `gtk::SortListModel` with a `gtk::MultiSorter` (directories first
  via `standard::type`, then `gtk::StringSorter` on `standard::display-name`), and
  `gtk::FilterListModel` for hidden files.
- Icons come from `GFileInfo::icon()` (content type icons), which deletes the extension
  table in `file_icon`.
- Context menu: `gtk::PopoverMenu`; reveal in file manager with
  `gtk::FileLauncher::open_containing_folder`; trash with `gio::File::trash_async`,
  which also removes the `trash` crate when GPUI goes.

**Delete:** `FilesModel`, `rows`, `make_row`, `watch`, the cap, the rebuild: about 330
lines. **Still write:** about 220: factory binding, open-file and "currently open" mark,
step-in and step-out keys, `reveal(path)` for "show this file in the tree", and the
state snapshot for the test scripts.

**User sees:** the same tree, with real file-type icons and a built-in expander look.
New folders appear without a manual refresh.

**Risks:**

- Keyboard: `GtkListView` handles Up, Down, Home, End, PageUp, PageDown and Enter
  (`activate`). `GtkTreeExpander` binds expand and collapse to keys such as Shift+Right
  and Shift+Left, `+`, `-` and `*`; I could not confirm the plain Left/Right behavior
  our tree has. If it is missing it is a ten-line key handler. Unverified.
- `reveal(path)` needs a walk down expanding each level, because rows do not exist until
  their parent is expanded. About 25 lines.
- Memory and latency are better than today: the list is virtualized and nothing is
  rebuilt on a change.
- `DirectoryList` shows what GIO lists. The finder already skips ignored folders; the
  tree lists "every entry" today, so no change.

**Verdict: replace.**

## 6. Git status and the watcher

**What it does.** Gives each workspace a branch, changed files, line counts and a dirt
tick in the sidebar. Current: `xenon_git` 1,014 lines (`load`, `measure`, `status`,
`bar`, `diff`), plus on A: `git.rs` 282, `watch.rs` 41, `git_ops.rs` 187,
`git_section.rs` 431. This entry is about the watcher and runner: `git.rs` +
`watch.rs` = 323 lines.

**Replacement for the data source:** keep the `git` command (`git status --porcelain=v2
-z --branch`, `git diff --numstat`). `git2` 0.21.0 would link the installed libgit2
1.9.1, and `gix` 0.90 is large; neither beats the command on big trees, and neither
removes the parsing we already wrote and tested.

**Replacement for the watcher:** `notify` watches the whole workspace recursively (one
inotify watch per directory), which is why `git.rs` has a ten-second poll fallback for
when inotify instances run out, plus a thread, a debounce and an `mpsc` channel, plus a
`[patch.crates-io]` pin on a fork.

- Watch only the few files that change when git state changes: `gio::File::monitor_file`
  (or `monitor_directory`) on `.git/HEAD`, `.git/index` and `.git/refs/heads`. Four
  inotify watches per workspace instead of thousands.
- Refresh also when a terminal in the workspace goes quiet (the attention logic already
  knows this moment), when the window gains focus, and after our own editor saves.
- Run git with `gio::Subprocess::newv` and `communicate_utf8_async` (or
  `communicate_async` for bytes), awaited with `glib::spawn_future_local`. That runs on
  the main loop without our own thread, channel or `MIN_GAP` logic.

**Delete:** the worker thread, the recursive watcher, the inotify fallback, the `notify`
dependency and its fork pin, `watch.rs`'s path-noise rules: about 170 lines. **Still
write:** about 150: the three monitors, the triggers, the async run, and `GitStatus`
plumbing.

**User sees:** the sidebar dirt tick updates when a git command runs, when a terminal
goes quiet, on focus, or after a save, instead of 400 ms after any file change. During a
long agent burst that edits many files the tick lags until the agent pauses. It also no
longer breaks when the system is out of inotify instances.

**Risks:** the lag during a burst is the one behavior loss; whether it matters is a
product call. CPU gets better: no watcher thread and no wakeups from build output in
`target/`.

**Verdict: partial.** Keep the git command and `xenon_git` parsing; replace the watcher
and the thread with GIO.

## 7. Diff view

**What it does.** A diff tab for one file: HEAD against the working tree (or staged
against unstaged), side by side or unified, with changed words marked. Current: on A,
`diff_view.rs` 693 and `diff_text.rs` 208, plus `xenon_git/src/diff.rs` 414 (+15 on A)
which parses `git diff` text, pairs removed and added lines into rows, and finds changed
words with a hand-written tokenizer. About 1,330 lines.

**Replacement, option A (recommended):**

- Compute the diff from the two texts with the `similar` crate 3.2.1
  (`TextDiff::from_lines` for rows, `iter_inline_changes` for changed words). Read
  HEAD's text with `git show HEAD:path`. This replaces `parse_unified`, `pair_rows`,
  `changed_words` and `tokens` (about 250 lines) by about 80. `imara-diff` 0.2.0 is
  faster but has no inline-change helper, so `similar` fits better.
- Draw each side as a `sourceview::View` over a `sourceview::Buffer` with the file's
  language set, so colors come from the editor's scheme. Put both views in a `gtk::Box`
  and give them the same `gtk::Adjustment` (`ScrolledWindow::set_vadjustment` on both)
  so scrolling is synced by GTK with no code.
- Row backgrounds: `sourceview::Buffer::create_source_mark` with categories "added",
  "removed", "filler", and `sourceview::MarkAttributes::set_background`, plus
  `View::set_mark_attributes`. GtkSourceView paints these as whole-line backgrounds.
  Changed words stay as ordinary `gtk::TextTag`s.
- Old and new line numbers: `diff_text.rs` builds a second text column for them. A
  `sourceview::GutterRendererText` subclass with the numbers is the built-in way (one
  GObject subclass, about 60 lines in gtk-rs; the subclass API works but I did not check
  this one).

**Replacement, option B (smaller, loses features):** put `git diff` output in one
read-only `sourceview::View` with the stock `diff` language. GtkSourceView colors added
and removed lines and headers itself. The whole diff tab is about 100 lines and
`xenon_git/diff.rs` and `diff_text.rs` go away: a saving of about 1,200 lines. It loses
side-by-side, word-level marks and the hunk navigation chrome.

**Delete (A):** about 580 lines. **Still write (A):** about 750.

**User sees (A):** the same layouts with GtkSourceView's gutter look; (B) a plain
unified diff.

**Risks:** A needs a gutter subclass and a scroll-sync spike; sync between views of
different line wraps drifts if wrapping is on (turn wrap off on diff views). `similar`
is mature (240M downloads). The three-way HEAD / index / working-tree logic in `load.rs`
stays either way.

**Verdict: partial (A).** B is the cheapest choice if the user will give up
side-by-side.

## 8. Ctrl+P finder and fuzzy ranking

**What it does.** Cached file index of the workspace, fuzzy ranking with `nucleo`,
`.gitignore` aware walk, recent files boost. Current: `xenon_finder` `lib.rs` 252,
`walk.rs` 131, `tree.rs` 48, `remote.rs` 13, and `xenon_gtk/src/finder.rs` 184; about
630 lines (discovery and tests are counted elsewhere).

**Replacement:** `nucleo::Nucleo` (the full crate we already depend on, version 0.5.0)
is designed for this: it owns a worker thread pool, takes items through an `Injector`
while a walk is still running, re-ranks incrementally as the pattern changes, and calls
back when results change. Feed it with `ignore::WalkBuilder::build_parallel` (`ignore`
0.4.34, which we already use). We currently use only `nucleo`'s `Matcher` and write our
own thread, 40 ms debounce, epoch counter and partial-snapshot callbacks around it.
`fuzzy-matcher` 0.3.7 is older and slower. Desktop indexers (Tracker or LocalSearch over
D-Bus) ignore `.gitignore` and may not be installed.

**Delete:** the snapshot machinery, debounce, epoch handling: about 160 lines. **Still
write:** about 470: the walk, the recent-file boost (a small sort after the top N from
`nucleo`), hit positions for highlighting, and rows.

**User sees:** results start appearing during the first walk and stay current.

**Risks:** `Nucleo` does not know about "recent files first". Take the top few hundred
from its snapshot and re-sort by our tier. Its callback fires from a worker thread, so
it needs an `async_channel` or `glib::MainContext::invoke` to reach the UI, a small
bridge. Latency should improve, since typing no longer waits for a debounce.

**Verdict: partial.**

## 9. Pickers and the command palette

**What it does.** One overlay shell (scrim, text field, result list) shared by Ctrl+P
and Ctrl+Shift+O. Current: `palette.rs` 469 lines. The command palette from GPUI
(Ctrl+Shift+P, about 480 lines) is not in GTK yet.

**Replacement:** `adw::Dialog` (1.5) in place of the scrim: it gives the dim backdrop,
Escape to close, focus restore to the previously focused widget, and an adaptive bottom
sheet on small windows. `gtk::SearchEntry` supplies `search-delay`, `search-changed`,
and `next-match` / `previous-match` signals (Ctrl+G and Ctrl+Shift+G by default). Keep
the `gtk::ListView` over a `gio::ListStore` that is already there. There is no built-in
command palette widget in GTK 4.22 or libadwaita 1.9. For Up, Down, Ctrl+N and Ctrl+P a
key controller on the entry still has to move the selection: about 25 lines.

**Delete:** the scrim, focus restore and open/close epoch bookkeeping: about 80 lines.
**Still write:** about 390.

**User sees:** a centred dialog with libadwaita's shadow and animation instead of a
panel on a scrim.

**Risks:** `adw::Dialog` animates in, which costs a few frames before the field accepts
typing. Check with `scripts/xenon-gtk-latency`; if it adds visible lag, keep the
overlay. Focus must stay in the entry while the list moves (our palette already does
this).

**Command palette on top:** list the `app.*` actions (`gio::ActionGroup::list_actions`),
look up each label in one table that also feeds the toolbar tooltips, and show the
accelerator with `gtk::Application::accels_for_action`. About 100 lines in all.

**Verdict: partial.**

## 10. Workspace picker (Ctrl+Shift+O)

**What it does.** Fuzzy over known workspaces first, then folders found under project
roots, with path completion and `ssh://` completion. Current: `picker.rs` 337 lines and
`xenon_finder/src/discover` (571 lines with tests).

**Replacement:** `gtk::FileDialog::select_folder` is already used for "Open workspace".
It runs through the desktop portal (a native chooser with bookmarks, recents, typing a
path with Ctrl+L, and a create-folder button). `gtk::RecentManager` could hold recent
workspaces.

**Delete:** nothing, unless the user accepts a mouse-first, portal-latency flow in place
of the keyboard picker. Then about 900 lines. **Still write:** the picker as it is.

**User sees:** (if replaced) a system dialog; the type-three-letters-and-Enter jump goes
away.

**Risks:** keyboard-first is a product rule here, and the portal chooser opens in
hundreds of milliseconds. The "New workspace" dialog (45 lines in `dialogs.rs`) can go,
since `FileDialog` has a create-folder button.

**Verdict: keep.**

## 11. Settings

**What it does.** Seven pages (Appearance, Fonts, Editor, Terminal, Agents, Language
Servers, Phone Remote) with live apply. Current: `prefs*.rs` 1,563 lines (of which
themes are counted in the theming entry, leaving about 1,005), `xenon_settings` 655 (of
which `themes.rs` 103 and `families.rs` 82 are counted elsewhere), `xenon_store`
`settings.rs` 263 and `prose.rs` 135.

**Replacement:** the UI is already `adw::PreferencesDialog` with `adw::ComboRow`,
`SwitchRow` and `SpinRow`; search is built in. The remaining question is storage.

- GSettings: `gio::Settings::bind(key, &widget, "active", ...)` would delete the per-row
  `connect_*_notify` closures and the state echo in `prefs_state.rs`, but it needs a
  schema compiled by `glib-compile-schemas` and installed, a dconf backend, and gives up
  the `settings.json` that agents and the Xenon skill edit. Not worth it.
- Font rows: `gtk::FontDialogButton` / `gtk::FontDialog` with a monospace filter
  replaces the curated `FontChoice` combo and `FontProbe`.

**Delete:** about 170 lines (font rows, `FontChoice` building, part of `prefs_state`).
**Still write:** the clamps and healing in `xenon_settings` (they are small and tested),
and the Agents and Phone pages.

**User sees:** a standard font chooser instead of a curated family list.

**Risks:** the font chooser lists every installed family, so the "monospace only" filter
matters; `gtk::FontDialog::set_filter` takes a `gtk::Filter` on `pango::FontFamily`, so
a `CustomFilter` that checks `is_monospace()` works. Unverified for faces (as opposed to
families).

**Verdict: partial.**

## 12. Theming

**What it does.** Ten theme families (Zed-schema JSON, 288 KB in
`xenon_terminal/assets/`, about to be homeless) set the chrome colors, the terminal
palette and the editor colors, with a live-preview picker with Keep and Escape. Current:
`theme.rs` 188 (+112 on A), `prefs_theme.rs` 197, `prefs_picker.rs` 361,
`prefs_apply.rs` 78, `xenon_settings/src/themes.rs` 103: about 1,040 lines.

**Replacement:** the stock stack gives three layers, each with a built-in:

- Chrome: `adw::StyleManager` (`set_color_scheme` System / Light / Dark, `accent_color`
  which follows the desktop accent since 1.6, and `high_contrast`). libadwaita widgets
  recolor themselves.
- Editor: GtkSourceView style schemes. `sourceview::StyleSchemeManager::default()` lists
  `scheme_ids()`; the library ships `Adwaita`, `Adwaita-dark`, `classic`, `kate`,
  `solarized-*` and others (list not verified on this machine; the files are in the
  library). `buffer.set_style_scheme(...)` colors text, selection, current line and the
  diff view for free. Our own scheme would be an XML file.
- Terminal: VTE has no palette presets. `Terminal::set_colors(fg, bg, &palette)` takes
  16 colors, which we derive from the chosen scheme's named styles (about 40 lines).

**Delete:** the JSON loader, the Zed-key to CSS mapper, the picker page with its tiles
and preview/keep/revert state, the lineup in `xenon_settings`, and the JSON files: about
840 lines and 288 KB. **Still write:** about 200: one combo row of scheme names, the
light and dark pairing (follow `StyleManager::is_dark`), VTE palette from the scheme,
accent for our few custom widgets.

**User sees:** the ten families (One, Gruvbox and the rest) are replaced by what
GtkSourceView ships. The chrome follows the system accent and light or dark. The live
preview with Keep and Escape is lost, though changing a combo row is instant. This is
the biggest visible change in this document.

**Risks:** no technical risk. It deletes a user-visible choice, so it needs a yes. A
middle path: keep two or three families as hand-written GtkSourceView XML schemes plus a
VTE palette each, about 60 lines per family.

**Verdict: replace**, after the user accepts the lost themes.

## 13. Fonts

**What it does.** Lilex and IBM Plex Sans ship inside the binary (8 TTF files, 1.6 MB)
for GPUI; families are probed with Pango. Current: `xenon_settings/src/fonts.rs` 18,
`families.rs` 82, `PangoProbe` in `window.rs` (about 30). The GTK app never registers
the embedded files: `rg EMBEDDED_FONTS` finds only a GPUI caller.

**Replacement:** system fonts through fontconfig and Pango. If Lilex is wanted, install
the TTF files to `~/.local/share/fonts` in `scripts/release/install-gtk` (zero Rust).
Pango 1.57 has `pango::FontMap::add_font_file` (Pango 1.56 and later) if the app must
carry its own fonts; unverified in the 0.22 binding. Default to the system monospace and
UI fonts. For the pickers see the settings entry (`gtk::FontDialogButton`).

**Delete:** the files and about 110 lines (and 1.6 MB from the binary). **Still write:**
nothing.

**User sees:** the default font becomes the system monospace unless Lilex is installed
alongside.

**Risks:** none for speed. Editor and terminal metrics depend on the font being present;
the existing "falls back to Monospace" rule keeps working.

**Verdict: replace.**

## 14. Keymap and shortcut text

**What it does.** Default bindings plus the user's `keymap.json`, turned into GTK
accelerators; text for shortcuts in tooltips and the empty-state card. Current:
`xenon_keymap` 793 lines (`defaults.rs` 280 written as macOS glyphs and converted,
`chord.rs` 234, `display.rs` 163, `effective.rs` 82) and `xenon_gtk/src/keys.rs` 198,
with an `ACTIONS` table from GPUI names to `app.*` names.

**Replacement:** the GTK machinery is already the engine: `gtk::Application::
set_accels_for_action("app.new-terminal", &["<Control>n"])`, which also gives menu and
button accelerator hints. Display text comes from `gtk::accelerator_parse` plus
`gtk::accelerator_get_label` (already used in `window.rs::tip`) and, in the empty card,
`gtk::ShortcutLabel::new("<Control><Shift>p")`. For a "Keyboard Shortcuts" screen,
`adw::ShortcutsDialog` (libadwaita 1.8, binding 0.9.2 with `v1_8`) takes sections and
items with accelerator strings and does the layout and search. `gtk::ShortcutController`
is for widget-scoped shortcuts; app-wide accelerators don't need it.

**Delete:** the macOS glyph converter, the display override map, `Platform`, chord
parsing (GTK accelerators cannot be chords anyway; `keys.rs` already returns `None` for
them), the `ACTIONS` rename table: about 640 lines. **Still write:** about 350: defaults
authored directly as `(action, accelerators)` pairs in GTK syntax, the `keymap.json`
reader (kept in its current Zed-like format so users' files still work), the two extras,
and the shifted-key variant list.

**User sees:** same keys. A new Keyboard Shortcuts dialog from `adw::ShortcutsDialog`
and keycap-styled shortcut labels. If we also change the `keymap.json` format to `{
"app.new-terminal": ["<Control>n"] }`, the file shrinks to about 220 lines in all but
users' existing files break; I would not do that.

**Risks:** low. The GPUI action vocabulary (`xenon::NewTerminal`) lives in users' files;
rename once and keep a small alias table for old names.

**Verdict: partial.**

## 15. Find in terminal

**What it does.** A search strip over the terminal: next, previous, close, smartcase.
Current: `find.rs` 135 lines on top of VTE's search.

**Replacement:** VTE already searches (`Terminal::search_set_regex`, `search_find_next`,
`search_find_previous`, `search_set_wrap_around`). The strip can be a `gtk::SearchBar`
holding the `gtk::SearchEntry`: it provides the reveal animation, Escape to close, and
`connect_entry`. Do not set `key-capture-widget` on the terminal, or typing in the shell
would open the bar.

**Delete:** the revealer and close handling: about 60 lines. **Still write:** about 75:
the smartcase pattern, the three buttons, and the VTE calls.

**User sees:** the same bar; libadwaita's look.

**Risks:** low. `VteRegex` flags (PCRE2 `MULTILINE` required) stay.

**Verdict: partial.**

## 16. Terminal links

**What it does.** Ctrl+click on a URL, an OSC 8 hyperlink or a file path, with `:line`
support. Current: `links.rs` 88 lines, plus `open_link` in `dialogs.rs`.

**Replacement:** this already uses VTE: `Terminal::match_add_regex` with
`vte::Regex::for_match`, `match_set_cursor_name`, `check_match_at` and
`check_hyperlink_at`. VTE does not open links by itself, so the Ctrl+click gesture
stays. `gtk::UriLauncher` / `gtk::FileLauncher` could replace
`AppInfo::launch_default_for_uri` in `launch()` (about 10 lines).

**Delete:** about 10 lines. **Still write:** the rest.

**User sees:** nothing.

**Risks:** none.

**Verdict: keep.**

## 17. Terminal activity, attention and the pulse

**What it does.** A tab shows "working" while output bursts run, and an attention mark
after a bell or an inferred idle; workspace rows aggregate their tabs; the working mark
pulses. Current: `xenon_core` `attention.rs`, `attention_map.rs`, `activity.rs`,
`agent.rs` (249 lines), `xenon_gtk` `model_status.rs` 150, `events.rs` 233: about 630
lines.

**Replacement:** the inputs already come from VTE signals (`contents-changed`, `bell`,
`window-title-changed`, `child-exited`). VTE 0.84 also has `notification-received` (OSC
777 `notify`) and, only for shells that emit OSC 133, `shell-precmd` and
`shell-preexec`. I believe all three exist in 0.84 but did not check the headers; none
of them tells an agent that is waiting for input apart from one that is thinking, which
is why we have burst heuristics. The output of the rules moves to
`adw::TabPage::set_needs_attention` and `set_indicator_icon` (tab entry). For desktop
alerts when the window is not focused, `gio::Application:: send_notification` with a
`gio::Notification` is about 20 lines, new behavior. The pulse (10 Hz stepped opacity
from a timeout, in `events.rs`, `pane.rs` and `sidebar.rs`) could be a CSS `@keyframes`
animation on a class, but that drives the frame clock at the display rate while any tab
works, which is the wrong trade given the idle-CPU goal.

**Delete:** the pulse code, about 70 lines, if the working mark becomes static. **Still
write:** the rules. They are product policy, not toolkit gaps.

**User sees:** a steady dot instead of a pulse for "working".

**Risks:** `notification-received` could be a better attention signal for agents that
emit it (unverified which do). Worth a look before the heuristics grow.

**Verdict: keep** (drop the pulse).

## 18. Terminal tab body and VTE settings

**What it does.** One VTE widget per tab with colors, font, scrollback, pty spawn and
cwd. Current: `term.rs` 195 lines.

**Replacement:** it is a thin layer over `vte::Terminal`. Spawn uses `spawn_async`,
theming uses `set_colors` and `set_font`, zoom uses `set_font_scale`. Nothing further to
remove.

**Delete:** nothing. **Verdict: keep.**

## 19. Copy Clean (strip agent chrome from a selection)

**What it does.** Removes quote bars and box lines when copying. Pure text logic of
about 440 lines in `xenon_terminal` (not in GTK yet).

**Replacement:** none. VTE gives `Terminal::get_text_selected` and
`copy_clipboard_format`; the cleaning rules are ours. When ported, the text rules are
about 200 lines once the GPUI clipboard glue is gone.

**Verdict: keep.**

## 20. Editor (plain files)

**What it does.** Open, edit, save, find, soft wrap, Vim keys, syntax. Current:
`editor.rs` 181 lines (+14 on A, +101 on B) on GtkSourceView. The 18,800-line GPUI
editor is not carried over.

**Replacement:** GtkSourceView 5.18 already provides undo, syntax highlighting, line
numbers, `VimIMContext` (Vim mode), and `LanguageManager::guess_language`. More of it
can replace code that is still to be written:

- `sourceview::FileLoader` and `FileSaver` with a `sourceview::File`: character encoding
  detection, newline handling, atomic save, async I/O, and
  `File::is_externally_modified` / `is_deleted` for the "changed on disk" prompt. This
  removes the 4 MB and UTF-8 refusal (`check`, `load`) and the hand-rolled
  `std::fs::read`, and works on remote `GFile`s.
- In-editor find: `sourceview::SearchContext` + `SearchSettings`, with match
  highlighting and counts, plus a `gtk::SearchBar`.
- `sourceview::View::set_highlight_current_line`, `set_show_line_marks`, `Completion`
  with the built-in `CompletionWords` provider, `Map` (minimap), `SpaceDrawer`, `Hover`
  for tooltips.
- Dirty-close prompt: `adw::AlertDialog` in the tab's `close-page` handler.

**Delete:** about 40 lines of hand file I/O. **Still write:** about 210 in all (it grows
by 30), and gains: search, reload prompt, dirty prompt, encodings, big files, words
completion.

**User sees:** more features than the stand-in has today.

**Risks:** `FileLoader` streams, so very large files should still be capped for latency.
The Vim mode is GtkSourceView's smaller subset: no visual-block painting and a limited
`:s`; already accepted in the cut list.

**Verdict: keep** (small, and mostly the toolkit).

## 21. Markdown prose mode (branch B)

**What it does.** Prose, Split and Source views of one buffer, a paper look, task boxes,
images, markup hidden away from the caret. Current: `xenon_prose` 694 lines (`parse.rs`
382, `lib.rs` 219, `conv.rs` 67, `query.rs` 26), `prose.rs` 387 with `input.rs` 115 and
`style.rs` 125, `prose_tags.rs` 246, `prose_images.rs` 165, and `editor_auto.rs` 168
(test hooks): about 1,900 lines.

**Replacement:**

- Source mode is free: GtkSourceView's `markdown` language and any style scheme.
- WebKitGTK rendering (`webkit6` 0.6.1): runtime library installed, dev package not;
  heavy and cannot edit in place. No.
- GtkSourceView style schemes support a `scale` attribute on styles, which could size
  headings in a pure-scheme "Source with prose styling" mode, but cannot hide markers;
  this is a possible cheaper first mode, unverified.
- Spelling: `libspelling` 0.5.0 (crate) over the C library 0.4.9 (apt candidate, not
  installed) adds spell check to a `GtkSourceView`. A gain, not a replacement.
- Parsing: `pulldown-cmark` (our dependency) is right; there is no GTK Markdown model.

**Delete:** nothing worth the risk. Possibly `xenon_prose` becomes a module of
`xenon_gtk`, but it is pure and testable and I would leave it. **Still write:** all of
it.

**User sees:** no change; spell check could be added.

**Risks:** the work is on a branch; merge it before more deletion happens around it.

**Verdict: keep.**

## 22. Vim mode and syntax highlighting

**What they do.** Vim keys in the editor; highlighting for fifty languages. Current GTK
code: about 20 lines to switch the input method on.

**Replacement:** `sourceview::VimIMContext` and GtkSourceView language definitions,
already used. They remove the 4,140-line custom Vim engine and the 780 lines plus fifty
grammar crates of tree-sitter highlighting that GPUI carries.

**Risks:** language coverage of GtkSourceView 5.18 for Solidity, Zig, Elm, Svelte, Nix
and PowerShell is unverified. List ids once with `language_ids()`.

**Verdict: replace** (already done on the GTK side).

## 23. Language servers (LSP)

**What it does.** F12 go to definition, F8 diagnostics, document highlight for Rust and
TypeScript. Current: `xenon_lsp` 993 lines; nothing in `xenon_gtk` except a settings
page.

**Replacement for the client:** none. `lsp-types` 0.97 is already used. No GNOME library
provides a shared LSP client (GNOME Builder's is private to it). Heavier crates
(`async-lsp`, `tower-lsp`) bring an async runtime. **Replacement for the UI:** the
editor's built-ins do about two thirds of it: `sourceview::Mark` / `MarkAttributes` for
diagnostic icons in the gutter with tooltips, `sourceview::Hover` with a `HoverProvider`
for type info, `sourceview::Completion` with a `CompletionProvider` for completion.

**Delete:** nothing. **Still write:** about 350 lines of wiring, instead of the GPUI 870
plus drawing.

**Risks:** language servers use hundreds of megabytes, against the low-memory goal. The
cut list already asks whether to keep this.

**Verdict: keep** if the user keeps LSP; it is not a library question.

## 24. Phone remote: HTTP and WebSocket transport

**What it does.** A small web server on the tailnet that pairs a phone, lists
workspaces, streams terminal screens over WebSocket, takes uploads, and rate-limits
guesses. Current: `server.rs` 438, `ws.rs` 312, `http.rs` 144 (`mio` plus `tungstenite`
plus our own request parser): about 890 lines; the rest of `xenon_remote` is protocol
and rules.

**Replacement:** `libsoup` 3 (`soup3` crate 0.9.0): `soup::Server::listen`,
`add_handler` for REST routes, `add_websocket_handler` for the stream, and
`soup::WebsocketConnection` with `connect_message`, `send_text` and `send_binary`. It
runs on the GLib main loop, so there are no threads, no `mio` and no hand-written frame
polling. It also parses multipart and request bodies. The runtime library (3.6.6) is
installed; the dev package is not, so `libsoup-3.0-dev` must be added to the build
dependencies. `axum` or `hyper` would bring `tokio` and dozens of crates. `tiny_http`
0.12.0 has no WebSocket.

**Delete:** about 540 lines and the `mio` and (with the IDE entry) `tungstenite`
dependencies. **Still write:** about 350: routes, pairing and rate-limit rules (kept in
`xenon_remote`), and handing messages across.

**User sees:** nothing.

**Risks:**

- Handlers now run on the GTK main loop. The VTE HTML parse is already done on a worker,
  but uploads and JSON handling must not block; use `spawn_blocking`-style threads for
  big bodies.
- Bind and tailnet checks (`network.rs`) stay ours.
- Unverified: `soup3` 0.9.0 against `glib` 0.22.10; check its `Cargo.toml` before a
  spike.
- One extra system package on every build machine.

**Verdict: partial** (replace the transport, keep the protocol). The most
dependency-heavy of the medium-risk items.

## 25. Phone remote: screen mirroring and pairing UI

**What it does.** While a phone is attached, dumps the VTE screen as HTML
(`Terminal::text_format(Format::Html)`), parses it off the UI thread, diffs against what
the phone has, and sends cell updates; pairing shows a QR code and six digits. Current:
`xenon_remote` `vte_html.rs` 223 and `frame.rs` 288, GTK `remote_mirror.rs` 221,
`remote_session.rs` 379, `remote_fit.rs` 147, `remote_host.rs` 197, `remote.rs` 296,
`remote_ui.rs` 314, `prefs_phone.rs` 279, and the 1,783-line `page.html`.

**Replacement:** the mirror already leans on the VTE built-in. Sending raw pty bytes to
xterm.js on the phone would delete `vte_html.rs`, `frame.rs`, `remote_mirror.rs` and
most of `page.html`, but VTE provides no way to see the child's output stream. The only
route is to own the pty and call `Terminal::feed` ourselves, which gives up VTE's pty
handling. For the QR code `qrcode` 0.14.1 is the right crate; GTK has none.

**Delete:** nothing. **Verdict: keep.** Alternatives cost more than they save.

## 26. Claude IDE WebSocket server

**What it does.** A local WebSocket server so Claude Code (and a Codex context socket)
can open files and read the selection, with a lock file per port. Current: `xenon_ide`
918 lines (`lib.rs` 346, `endpoint.rs` 214, `codex_protocol.rs` 168, `protocol.rs` 140,
`lock.rs` 50); not wired into GTK.

**Replacement:** the same `soup::Server` WebSocket handler as the phone remote, which
would let both servers drop `tungstenite` and `mio` and run on the main loop without a
thread per client. The MCP and JSON-RPC protocol code does not shrink.

**Delete:** about 150 lines (threads and framing). **Still write:** the rest.

**Verdict: partial, or cut.** `PRODUCT.md` calls the bridge "optional garnish" and
`xenon open` covers opening files. If kept, do it on libsoup together with the phone
transport; if the transport stays on `tungstenite` for the phone, leave this alone.

## 27. SSH workspaces

**What it does.** Browse and edit remote text files and reopen persistent tmux
terminals. Current: `xenon_ssh` 522 lines (`ssh_config.rs` 164, `lib.rs` 139, `host.rs`
101, `transport.rs` 70) plus `helper.py` 280 and `helper_test.py` 163 run on the remote
host through `ssh -o ControlMaster=auto`. GTK has no UI yet (it shows an "unsupported"
page). The GPUI UI for it was about 1,090 lines.

**Replacement:** GIO's `sftp://` backend. A remote path is a `gio::File::for_uri
("sftp://host/path")`, and everything GTK already does with a `GFile` works on it:
`gtk::DirectoryList` (the Files tree, with remote change monitoring where the backend
supports it), `sourceview::FileLoader` / `FileSaver` (the editor, including
`is_externally_modified`), `gio::File::query_info_async`. `gvfsd-sftp` starts the system
`ssh` binary, so `~/.ssh/config`, agent and `ProxyJump` apply. The password or host-key
prompt comes through `gtk::MountOperation`. Persistent terminals stay as a VTE tab
running `ssh host -t tmux ...`.

**Delete:** `helper.py`, `helper_test.py`, `transport.rs`, the revision protocol: about
745 lines, and it avoids the 1,090-line GPUI-style UI. **Still write:** about 220:
`ssh_config.rs` host completion, mounting, tmux terminal command, and the finder for
remote trees (`find` or `git ls-files` through `ssh`, since `ignore` cannot walk a
`GFile`).

**User sees:** remote files in the same tree and editor, with the desktop's own mount
prompts. Editing over sftp has more per-save latency than the helper.

**Risks:**

- GVfs is not installed on this machine (`dpkg -l` finds no `gvfs*` package), and
  without it `sftp://` does nothing. The desktop here appears to include KDE components,
  which use KIO instead. Installing `gvfs-backends` is one package, but it is a runtime
  requirement that must be stated in the README.
- GVfs mounts can block the main thread if a call is made synchronously; use only the
  `_async` GIO calls.
- No conflict revisions: the helper's save protocol detects remote edits; `FileSaver`
  uses an etag, which covers the common case.
- Spike first: mount a real host, open a file, save, drop the network.

**Verdict: replace if GVfs is acceptable**, otherwise keep the helper. This is a
decision about a runtime dependency.

## 28. `xenon open`, the socket and single-instance handling

**What it does.** `xenon open FILE[:LINE] [--pane ...]` tells the running instance to
open a file; a second launch with folders forwards to the first. Current: `xenon_store`
`cli.rs` 170, `ipc.rs` 233, `open_spec.rs` 34 (437 lines); the GPUI server is not
counted. The GTK app already uses `adw::Application` with `HANDLES_OPEN`, so folders
given on the command line reach the running instance over D-Bus today.

**Replacement:** `GApplication` itself.

- Register `HANDLES_COMMAND_LINE` and use `connect_command_line`; the remote
  `ApplicationCommandLine` gives `arguments()`, `cwd()`, `environ()`, `print()` and
  `set_exit_status`, so the command-line parser runs in the first instance and the
  second one just forwards and exits. Or define a `gio::SimpleAction` `open-file` with a
  parameter such as `(sii)` and call it from a tiny client with
  `gio::Application::activate_action` (also usable from the shell as `gapplication
  action dev.xenon.Gtk open-file "('/path', 12, 0)"`).
- A tiny `xenon` binary that links only GLib/GIO (no `gtk::init`, no display) does the
  forward in about 40 lines, so it also works from an SSH session. Whether
  `adw::Application` forwards a command line before GTK init needs a check (I believe
  the remote path never calls `gtk_init`, since it happens in `startup`, but did not
  verify).
- With a `.desktop` file that has `DBusActivatable=true`, `xenon open` starts the app if
  it is not running, which today's socket cannot do.

**Delete:** `ipc.rs`, the socket server and its timeouts, the per-data-dir socket name,
the `XENON_DATA_DIR` injection into shells: about 300 lines. **Still write:** about 140:
argument parsing for `FILE:LINE`, `--pane`, the action, and the forwarder.

**User sees:** the same command. No socket file to go stale.

**Risks:**

- Needs a session D-Bus where agents run. Terminals inside Xenon inherit it; a detached
  container or a bare SSH session may not have one.
- Start-up time of the forwarder: a GLib-only binary should be a few milliseconds, but a
  full `adw::Application` start would not be; keep the client GTK-free.
- A/B slots stop mattering: the app id separates instances (`XENON_GTK_APP_ID` already
  does).

**Verdict: replace.**

## 29. Command-line parsing (`clap`)

**What it does.** `xenon-gtk [--reseed] [--screenshot PNG] [DIR...]` today; `xenon open`
per the entry above. Current: `parse_args` in `main.rs`, about 20 lines.

**Replacement:** `gio::Application::add_main_option` (GOption) gives `--help` and
`--version` and type-checked values, handled by GLib before `activate`. `clap` 4.6.7
would add roughly a dozen crates and compile time for a few options.

**Delete:** about 10 lines, at most. **Verdict: keep.** Do not add `clap`.

## 30. Session save and restore, window state, state storage

**What it does.** Saves the registry, each workspace's pane layout, window size and
sidebar width under `~/.xenon`; restores them at start. Current: `persist.rs` 88,
`xenon_core` `session.rs` 71 and `workspace.rs` 141, `xenon_store` `lib.rs` 206,
`settings.rs` 263 and others: about 600 lines in the paths that matter here.

**Replacement:** no GTK 4 session manager exists (`GtkApplication` session support is
for logout inhibit only). `gio::File::replace_contents_async` gives an atomic write with
an etag, which could replace a hand-written temp-and-rename if `xenon_store` has one.
XDG paths come from `glib::user_config_dir()` and `user_state_dir()`; moving off
`~/.xenon` would break the skill, the docs and agents that know the path, for no code
saved. GSettings is covered above.

**Delete:** the A/B slot code (`SLOT_DIRS`, `shared_dir`, the slot detection in
`remote_devices.rs`), about 80 lines, which is cutover work and not a library swap.
**Still write:** all else.

**Verdict: keep.**

## 31. Cutover leftovers

**What they are.** `data.rs` (98 lines) seeds `~/.xenon-gtk` from the GPUI slot and
handles `--reseed`; `show_unsupported`; `ACTIONS` rename table; `Highlighter` hook
parameters; the `macos/xenon-icon-source.png` path that branch A's `empty.rs` includes
with `include_bytes!`. About 180 lines.

**Replacement:** deletion after cutover. Note that A's empty state reads an image from
`macos/`, which the cut list deletes; install a themed icon (`dev.xenon.Gtk` in
`hicolor/`) and use `gtk::Image::from_icon_name` or `adw::StatusPage::set_icon_name`
instead.

**Verdict: delete.**

## 32. Empty state

**What it does.** Teaching text and a short command list when nothing is open. Current:
`empty.rs` 37 lines on main, 132 on A (a hand-built card).

**Replacement:** `adw::StatusPage` (icon name, title, description, and a `child`) with a
`gtk::ListBox` of `adw::ActionRow`s, each with a `gtk::ShortcutLabel` suffix from the
keymap accelerators. The buttons use the existing `app.*` actions with
`suggested-action` / `pill` classes. This removes the glyph-to-text conversion of `⌘⇧P`
strings.

**Delete:** about 80 lines. **Still write:** about 50.

**User sees:** libadwaita's centered status page with keycaps instead of the old card.
Text stays word for word.

**Risks:** none.

**Verdict: replace.**

## 33. Toolbar and custom icons

**What it does.** Header buttons for sidebar, split right, split down and find. On A, it
adds two SVGs, a `.gresource.xml` and a `build.rs` that runs `glib-compile-resources`.

**Replacement:** the Adwaita icon names already used on main (`view-dual-symbolic`,
`view-paged-symbolic`, `sidebar-show-symbolic`, `system-search-symbolic`). The tooltip
accelerator text in `window.rs::tip` is about 12 lines and has no built-in (neither GTK
nor libadwaita adds accelerators to tooltips).

**Delete:** the SVGs, resource XML and `build.rs` from A: about 30 lines and a build
step that needs `libglib2.0-dev-bin`. **Verdict: replace.**

## 34. Toasts and dialogs

**What they do.** Transient messages; confirm and entry dialogs; folder chooser.
Current: `commands.rs::toast` (5 lines), `dialogs.rs` 130 lines.

**Replacement:** already `adw::ToastOverlay` and `adw::Toast`, `adw::AlertDialog`,
`gtk::FileDialog`. Toasts are done. In `dialogs.rs`, the "New workspace" dialog (about
45 lines) can go if `FileDialog::select_folder`'s own create-folder button is enough
(its typed-path flow is the loss). `gtk::UriLauncher` / `FileLauncher` replace the
`launch()` helper's `AppInfo` call.

**Delete:** about 50 lines. **Verdict: partial.**

## 35. Memory diagnostics

**What it does.** A panel with the app's footprint and per-terminal grid estimates,
written to JSON. Current: `xenon_memory` 133 lines (GPUI panel 211, none in GTK).

**Replacement:** none needed. `scripts/xenon-gtk-idle` reads `/proc`, the system monitor
shows memory, and `GTK_DEBUG=interactive` opens the GTK Inspector (object counts, CSS,
widget tree) in any run.

**Delete:** the crate, 133 lines. **Verdict: delete.**

## 36. VS Code `tasks.json`

**What it does.** Parses `.vscode/tasks.json`, resolves variables and inputs, runs a
task in its own tab, asks consent for `folderOpen` tasks. Current: `xenon_core/src/
tasks/` 1,524 lines plus `task_trust.rs` 64; no GTK UI.

**Replacement:** none. No library implements VS Code task semantics. The runner part is
`vte::Terminal::spawn_async`, which GTK already uses for shells. A JSON-with-comments
parser (`serde_json_lenient`) is already a dependency.

**Delete:** nothing by library. If the cut list removes tasks, 1,588 lines go.
**Verdict: keep, or cut as a product decision.**

## 37. Worklist panel (not in GTK yet)

**What it does.** Quick capture (Ctrl+Shift+K), a list tab for `.xenon/worklist.md` with
sections, check-off and edit. GPUI is about 2,250 lines; the file-format parser
(`worklist_file/`, 966 lines) is GPUI-free.

**What to build it from:** the file is plain Markdown, so open it as an ordinary editor
tab. With prose mode (branch B) checkboxes already toggle; the editor has find and Vim.
For capture, an `adw::Dialog` with a `gtk::TextView`, bound to an action, appends `- [ ]
text` to the file with `gio::File::append_to_async`. If a list view is wanted later, a
`gtk::ListView` of `adw::ActionRow`s with a `gtk::CheckButton` suffix over a
`gio::ListStore` fed by the existing parser works, but it is not needed to start.

**Still write:** about 60 lines. **Avoids:** about 2,200.

**User sees:** the file opens like any note, and Ctrl+Shift+K still captures. The
sectioned list tab goes away.

**Risks:** none on the toolkit side; the 966-line parser can then be dropped too if
nothing else needs it.

**Verdict: replace.**

## 38. Image viewer, back and forward, command palette (not yet in GTK)

Covered in the table "Not yet in GTK". In brief: images use `gtk::Picture::set_file`
inside a `gtk::ScrolledWindow` (about 40 lines instead of 700; it decodes through
gdk-pixbuf or glycin); back and forward history has no built-in and stays a small list
in the model; the command palette is the existing shell plus the action list.

## 39. Test and diagnostics hooks

**What they do.** D-Bus actions that return JSON (`ui-state`, `remote-state`,
`editor-state`, `editor-do`), the palette snapshot, a PNG screenshot action, the latency
trace. Current: `latency.rs` 114, `screenshot.rs` 39, `prefs_state.rs` 116 (publishing),
`model_describe.rs` 60, `editor_auto.rs` 168 (B), and the scripts (`xenon-gtk-verify`
895 lines, `idle` 263, `latency` 120, `remote-client` 243).

**Replacement:** the GTK Inspector and `GTK_DEBUG` cover debugging by a human, and
`dogtail` or AT-SPI cover accessibility-driven tests, but our scripts need exact state
as JSON, and Wayland has no portable key injection. State-as-action is the supported way
to expose it.

**Delete:** nothing worth it; these are what let agents check the UI without a human.
**Verdict: keep.**

---

# Open questions for the user

1. Themes: accept the stock GtkSourceView schemes and system accent in place of the ten
   Zed families? (Saves about 840 lines. Largest visible change.)
2. SSH: is GVfs (`gvfs-backends`) an acceptable runtime requirement? (Saves about 745
   lines and removes the Python helper.)
3. Diff: keep side-by-side with changed words (option A, about 750 lines) or move to a
   unified diff only (option B, about 100)?
4. Git dirt tick: is a refresh on git changes, terminal quiet, focus and save enough, or
   must it follow every file edit?
5. Sidebar: is a draggable width needed? If not, `adw::OverlaySplitView` removes the
   `Paned` and the saved width.
6. Phone remote: add `libsoup-3.0-dev` to the build dependencies in exchange for about
   540 fewer lines and no `mio` or `tungstenite`?
