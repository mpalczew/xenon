# Terminal service

A long-lived service, `xenon-term`, runs and holds every terminal. The GTK window and
the phone page are clients of it. Restarting or upgrading the window leaves every shell
and agent running. Sketches A, B and C were in `/tmp/xenon-arch/`. This design is built:
`crates/xenon_term`, with the window's side in `crates/xenon/src/{term,term_events}.rs`.
The spike settled the main risk: VTE 0.84 emits `commit` with no PTY for everything
bound for the child (vte.cc:4776-4802), and replay through VTE is exact.

## Problem

Today VTE owns each PTY, child and screen inside the GTK process, so quitting the window
kills every agent. The a/b slots in `scripts/xenon` work around this. Grounding brought
up these constraints:

- **VTE stays the desktop renderer.** It handles find, links, OSC 8, selection, zoom,
  and the bell, title and output signals that drive `Model`. A new window process starts
  with a blank VTE, so the service has to keep a screen of its own to repaint from.
- **A PTY master has one reader.** The service reads it and fans the output out to every
  client. So handing the PTY fd to the UI (SCM_RIGHTS) can't work: there would be no
  phone and no repaint.
- **The phone has to work while the window is down.** Today it scrapes VTE
  (`remote_mirror.rs`, `vte_html.rs`), and it can't see cursor visibility or mouse mode.
- **The phone never moves the computer** (`mobile-remote-v2.md`).
- **Least code.** Use libraries (`alacritty_terminal`) and the platform (a systemd user
  socket) rather than writing our own.

## Usage (caller's view)

The UI links `xenon_term` (no GTK). The phone server runs inside the service and calls
the same `Terms` API in-process. Only the window crosses the socket.

```rust
use xenon_term::{Client, Spawn, Size, TermId, Event, Claim};

// app.rs, startup. connect() starts the service through socket activation if needed.
// It sends this window's environment as the base for every spawn, including spawns
// made from the phone.
let svc = Client::connect(child_env())?;
let live = svc.list()?;                       // Vec<TermInfo>: id, workspace, pid, status
for tab in model.terminal_tabs(ws) {          // TabState::Terminal now stores `term: Option<TermId>`
    let id = match tab.term.filter(|id| live.iter().any(|t| t.id == *id)) {
        Some(id) => id,
        None => svc.create(ws, Spawn::shell(&root))?,   // service restarted: a fresh shell, as today
    };
    views.insert(id, TermView::new(app, ws, tab.id, id));
    svc.attach(id, view.size());               // on first map; a replay arrives first as Event::Output
}
// The adoption rule: any live terminal in `ws` that no tab references gets a new
// unselected tab. This one rule covers phone-made terminals and terminals left behind
// by a crashed window.

// events.rs: one receiver, drained on the GLib main loop.
while let Ok(ev) = svc.events().recv().await {
    match ev {
        Event::Output { id, bytes } => app.view(id).term.feed(&bytes),
        Event::Status { id, status } => app.term_status(id, status),   // dots, title, exit
        Event::Size { id, size } => app.view(id).follow(size),         // pin VTE if smaller
        Event::Created { id, workspace } => app.adopt(workspace, id),  // e.g. from the phone
        Event::Closed { id } => app.term_closed(id),                   // e.g. closed from the phone
        Event::Phone => app.remote_publish(),                          // phone server state changed
        Event::Lost => app.service_lost(),
    }
}

// term.rs: VTE has no PTY. `commit` carries keys, pastes, mouse reports and VTE's own
// replies to terminal queries.
term.connect_commit(move |_, text, _| svc.input(id, text.as_bytes()));
// The tab is on screen: clears attention, as switching to the tab does today.
svc.seen(id);
// Closing the tab hangs up the child.
svc.close(id);
```

```rust
// xenon_term/src/phone.rs (inside the service): the phone is a client of `Terms` too.
let w = terms.watch(id, conn, WatchRate::Normal)?;  // per-phone viewport, frames via callback
w.scroll(-5);                                       // per phone; never moves the computer
terms.claim(id, conn, Claim::Phone { size, label });   // the fit gate merges the claims
terms.create(ws, Spawn::shell(&root))?;             // window adopts it unselected
```

## Shape

### Crates

| Crate                      | Change                                                                                                                                                                                                                                                 |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `xenon_term` (new, no GTK) | `Client` (the window's side of the socket), `Terms` (the service core), `run_service()`, private `wire`, `emu` (alacritty_terminal plus replay), `pty`, `phone` (adapter to `xenon_remote`)                                                            |
| `xenon_remote`             | Keeps HTTP, WS, pairing, devices and frames. Its `HostRequest` is now answered by `xenon_term::phone`, not the GTK app. `vte_html.rs` is deleted; `Screen` is built from emulator cells                                                                |
| `xenon` (GTK)              | `term.rs` becomes a fed view. `remote_mirror.rs`, `remote_host.rs`, `remote_session.rs` and `remote_fit.rs` are deleted, and `remote_tasks.rs` moves into the service. `remote.rs` and `prefs_phone.rs` become status and settings views over `Client` |
| `scripts/xenon`            | The a/b slots go. It installs and enables `xenon-term.socket`, then runs the UI                                                                                                                                                                        |
| `src/bin/xenon-term.rs`    | `fn main() { xenon_term::run_service() }`                                                                                                                                                                                                              |

### Data (service, private)

```rust
struct Terms {
    base_env: Vec<String>,                    // last window's env; only Client::connect writes it
    terms: HashMap<TermId, Term>,
}
struct Term {
    workspace: WorkspaceId,                   // set once at create; the service reads the
                                              // registry file only for names and roots (phone)
    pid: i32,
    master: OwnedFd,                          // read by this terminal's own thread only
    emu: Emu,                                 // alacritty_terminal::Term, 10k scrollback
    status: TermStatus,                       // derived here from output, bell, input and exit
    claims: Claims,                           // per actor: Desk(size), Phone{conn,size,label}
    desk: Option<ConnId>,                     // the one attached window; it answers queries
    watchers: Vec<Watcher>,                   // phones: own scroll, rate, dirty flag
}
pub struct TermStatus { pub working: bool, pub attention: Option<AttentionReason>,
                        pub title: String,            // already cleaned, ready for a tab label
                        pub exit: Option<Exit>,
                        pub phone: Option<String> }   // the phone whose size is in force
```

These are the main access paths through it:

- **Output.** The thread reads the master, calls `emu.advance`, and sends the raw bytes
  to `desk`. It marks watchers dirty and updates `status` through the existing
  `xenon_core::TerminalActivity` state machine, which moves here from the UI.
- **Attach.** Under the terminal's lock, it renders `emu.replay()` (standard VT: RIS,
  scrollback and screen with SGR and OSC 8, soft wraps, the alternate screen, modes,
  cursor, OSC 2 title), queues it as the first `Output`, then adds the desk. No byte is
  lost or sent twice. A second attach replays again, so it is idempotent. A window that
  falls more than 4 MiB behind is dropped and repainted when it catches up.
- **Query replies.** If `desk` is set, the emulator's replies are dropped, because VTE
  answers through `commit`. If it isn't, the service writes them to the PTY. One
  answerer at a time.
- **Status.** `seen(id)` clears attention. The service does not know what is on screen,
  so it flags attention whenever a terminal rings the bell or settles, and the window
  calls `seen` at once for a terminal it is showing. Output counts as a wakeup at most
  every 16 ms, the rate a VTE redraw would.
- **Size.** `claims.effective()` is the desk size, unless a phone has claimed a size and
  the desk has had no input for 30 seconds (`FitGate`, as today); the smallest phone
  wins. It is derived at read time, so no one writes the size directly. The window
  follows with `Event::Size`.

### Public surface

```rust
pub struct TermId(u128);          // random, never reused: a stale id only misses
pub struct Size { cols: u16, rows: u16 }   // >= 1, checked in Size::new
pub struct Spawn { pub cwd: PathBuf, pub argv: Vec<String>, pub env: Vec<String> } // env is added to base_env;
                                          // the service adds XENON_WORKSPACE, TERM and COLORTERM
pub enum Exit { Code(i32), Signal(i32) }
pub(crate) enum Claim { Desk(Size), Phone { size: Size, label: String } } // the window is always the desk,
                                                                          // so `Client` takes a plain `Size`
pub struct TermInfo { pub id: TermId, pub workspace: WorkspaceId, pub pid: i32, pub status: TermStatus }
pub enum Event { Output{..}, Status{..}, Size{..}, Created{..}, Closed{..}, Phone, Lost }

impl Client {
    pub fn connect(env: Vec<String>) -> Result<Self, ConnectError>; // checks protocol version
    pub fn events(&self) -> async_channel::Receiver<Event>;
    pub fn list(&self) -> Result<Vec<TermInfo>, Gone>;
    pub fn create(&self, ws: WorkspaceId, spawn: Spawn) -> Result<TermId, SpawnError>;
    pub fn attach(&self, id: TermId, size: Size);
    pub fn input(&self, id: TermId, bytes: &[u8]);
    pub fn claim(&self, id: TermId, size: Size);
    pub fn seen(&self, id: TermId);
    pub fn close(&self, id: TermId);          // idempotent; SIGHUP to the session
    pub fn set_dark(&self, dark: bool);       // the phone page follows the window's theme
    pub fn phone(&self) -> PhoneLink;         // pairing code, status rows, settings, for prefs_phone
}
```

It hides the socket, framing, versioning, PTY setup, reaping, the emulator, replay,
query answering, size arbitration, activity timing, and the whole phone server. It still
exposes `TermStatus`, because tabs and the sidebar paint it, and `WorkspaceId`, because
the phone lists terminals by workspace and the window adopts terminals by it. It doesn't
know about panes, tab order, selection, editors or layout: those stay in the window's
`Model`.

The window keeps: `Model` (layout, selection, editors), the task UI for desktop runs (it
resolves a task and calls `create`), find, links, the context menu, clipboard, zoom, and
the pulse animation.

## Synthesis decision

The base is **A** (a service that holds only terminals). It has the smallest surface,
and the window's `Model` and layout code stay where they are.

- **From B:** the phone server moves into the service, so phones work with no window
  open. This was A's biggest gap and contradicts the request. Also from B: per-phone
  scroll, dropping and repainting a slow window, and status (dots) owned by the service,
  so the window and the phone read one source.
- **From C:** systemd socket activation. Size as per-actor claims merged at read time,
  instead of the window pinning VTE on the phone's behalf. Base env pushed on connect,
  so phone-made terminals get the desktop environment.
- **Rejected from A:** the opaque `Tag` (`TabState::Terminal` stores the `TermId`
  directly), a phone server inside the window, and the fd-store crash recovery
  (deferred; see below).
- **Rejected from B:** moving `Model`, layout and desk selection into the service. It
  doubles the size of the move and splits state between desk, shared and phone, and the
  gain (the window comes back in place) is already covered by today's session file.

## Tradeoffs accepted

- We accept a second process and a systemd user unit in exchange for terminals that
  outlive the window.
- We accept writing replay generation (about 300 lines) in exchange for exact repaints,
  links included, and no tmux dependency.
- We accept that a service crash or a service upgrade still kills terminals, as a window
  crash does today, in exchange for not building fd-store recovery and in-place re-exec
  now. Install doesn't restart the service; it upgrades on the next login or when you
  ask.
- We accept that the phone reads workspace names and roots from the registry file the
  window writes. The service reads it and never writes it.
- We accept that closing the window no longer ends agents. Ending them takes an explicit
  "Quit and end terminals".

## Alternatives considered

- **B, the service owns the session.** It is deeper in total but much wider: around 20
  intents plus a `SessionView` stream, and most of `crates/xenon` moves. It lost on cost
  against nearly the same benefit.
- **C, a stock tmux server holds the terminals.** It is the least holder code of the
  three. It lost on hidden cost: a control-mode client, a filter to stop tmux and VTE
  both answering queries, repaint through `capture-pane` (OSC 8 support unproven),
  status polled at one-second steps, and an external program's semantics to work around.
  It is the fallback if the replay spike fails.
- **Passing the PTY fd to the UI (SCM_RIGHTS)** has one reader, so no phone and no
  repaint. **dtach/abduco** keep no screen. **shpool** allows one client at a time.

## Decisions

- Closing the window keeps agents running; "Quit and end terminals" ends them.
- The service starts only through a systemd `--user` socket.
- Service upgrades are manual for now and end terminals; in-place re-exec comes later.
- State lives in `~/.xenon` only. The a/b slots and their `~/.xenon-a`/`-b` directories
  are gone; a new window never hands off to an old slot instance's `ipc.sock`.
- The window sends `Size` claims only while it is unpinned, and attaches a terminal on
  its first map, so a replay never lands in a VTE that has not been laid out yet.

## Limits

- **Mouse: SGR (1006) only.** VTE sends legacy X10 and normal mouse reports through
  `feed_child_binary`, which drops them when there is no PTY. Programs that never turn
  on SGR mouse get no mouse from the window. We do not work around it.
- **Replay is not exact for** the scroll region (DECSTBM), tab stops and character sets:
  `alacritty_terminal` does not expose them. Origin mode is replayed. A program that set
  a scroll region sets it again on its next redraw.
- **Focus.** The replay turns focus reporting (1004) on, and VTE answers with a focus
  report. The window drops that one report, so reattaching injects no focus event.
- **A service crash or upgrade ends every terminal**, as a window crash did before.
- **Phone tabs are numbered** by a counter that starts again when the service does.
- **Phone-run tasks always open a new terminal** (no shared or dedicated panel reuse, no
  folder-open tasks); the desktop keeps both.
- **Window colors for a phone with no window ever open** come from the built-in palette;
  the window sends only whether its theme is dark.
- The base environment is the service's own until a window connects.

## Open questions

- Does viewing a terminal on the phone clear its attention dot? Until decided, it does
  not (today's behavior).
