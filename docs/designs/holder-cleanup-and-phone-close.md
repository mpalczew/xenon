# Holder cleanup and closing a workspace from the phone

Status: built. Follows [terminal-holders.md](terminal-holders.md).

## Problem

Seven cleanups left from the holder work, and one feature: close a workspace from the
phone page. Constraints:

- Holders live for weeks. Nothing here may need a holder restart. Only item 7 changes
  holder code, and it reaches new holders only.
- The window is the only writer of `workspaces.json`. The phone server reads it and
  never writes it, and keeps no file of its own about workspaces.
- Builds differ until each process restarts. Install restarts the phone server, and the
  window restarts when the user chooses. So the phone server is normally the same build
  as the window or newer.

## Usage (caller's view)

Phone page, on an open workspace:

```js
if (!confirm(`Close ${w.name}? ${ending(w.terminals)}`)) return;   // "Its 3 terminals will end."
await api(`/api/workspaces/${id}`, { method: "DELETE" });          // 409: show its message
loadWorkspaces();                                                   // already saved by the window
```

Phone server, `host.rs`:

```rust
HostRequest::CloseWorkspace { workspace_id, reply } => {
    let _ = reply.send(self.close_workspace(&workspace_id));        // asks the window over ipc.sock
}
```

Window, `ipc.rs`, beside the existing requests:

```rust
IpcRequest::CloseWorkspace { workspace } => return self.close_from_phone(workspace),
```

Holder calls keep their public API. Inside, each is one line:

```rust
pub fn attach(&self, id: TermId, size: Size) { self.shared.links.send(id, Call::Attach(size)) }
```

## Shape

### Item 8: close a workspace from the phone

The phone server asks the window on the socket the window already serves for the CLI,
`ipc.sock` (`xenon_store::ipc`). There's no new socket and no new protocol.

- `IpcRequest::CloseWorkspace { workspace: WorkspaceId }`, additive.
- Window: `close_from_phone(ws) -> Result<Option<String>, String>`. It refuses with
  "unsaved changes in <file>" if an editor in the workspace is dirty. Otherwise it runs
  `close_workspace_id(ws)` and answers `Ok`. It does not raise the window, unlike the
  CLI requests. Closing a workspace that is not open answers `Ok`, so the request is
  idempotent.
- `close_workspace_id` ends every holder whose `TermInfo.workspace` is `ws`, taken from
  `holders.list()`, not just the ones with a view. Today a workspace not opened this
  session keeps its terminals after a close. From the phone, that is the usual case.
  This changes desktop behaviour too.
- The window answers only after the close has saved the registry. So when the page
  reloads the list, `workspaces.json` already shows the workspace as closed.
- Phone page: Close on each open workspace, in the workspace list and the terminals
  list. Phone server: `DELETE /api/workspaces/{id}` sends the request through
  `xenon_store::try_handoff_request`, on its own thread so the host thread never waits:

  | Result          | HTTP | Page shows                                                |
  | --------------- | ---- | --------------------------------------------------------- |
  | `Sent`          | 200  | list reloads                                              |
  | `NoServer`      | 409  | "Open Xenon on the computer to close this workspace"      |
  | `Rejected(msg)` | 409  | `msg` (unsaved changes, or an old window's "bad request") |

- A dirty tab of any kind blocks the phone close. The refusal names its file or title.

Restarts: the phone server restarts on install, and the window when the user chooses. No
holder restarts. Until the window restarts, a close from the phone gets the old window's
"bad request" refusal, shown as text.

### Items 5 and 6: one map, one ordered queue per terminal

`crates/xenon_term/src/links.rs` replaces `Shared.holders`, `.early` and `.connecting`:

```rust
pub(crate) struct Links(Mutex<HashMap<TermId, Slot>>);

enum Slot {
    /// No connection yet. Calls fold into `early`. `connecting`: one thread is adopting it.
    Waiting { early: Early, connecting: bool },
    Live { link: Arc<HolderLink>, out: mpsc::Sender<Call> },
}

pub(crate) enum Call { Attach(Size), Claim(Size), Input(Vec<u8>), Seen, Close }

/// Folded: the last size wins; attach and close are flags; input keeps its order.
/// Replayed as Claim, View, input, then Close.
#[derive(Default)]
struct Early { size: Option<Size>, attach: bool, input: Vec<u8>, close: bool }

impl Links {
    /// Only `open` (before it returns the id) and `scan` add entries.
    pub fn expect(&self, id: TermId);
    /// Live: push onto the queue, which never blocks. Waiting: fold. Unknown id: drop.
    pub fn send(&self, id: TermId, call: Call) -> bool;
    pub fn begin_connect(&self, id: TermId) -> bool;
    pub fn connect_failed(&self, id: TermId, forget: bool);
    /// Creates the queue, fills it with the early calls, then makes it reachable
    /// through the map. Any later call lands behind the replay.
    pub fn publish(&self, link: Arc<HolderLink>, stream: UnixStream);
    pub fn remove(&self, link: &Arc<HolderLink>) -> bool;
    pub fn get(&self, id: TermId) -> Option<Arc<HolderLink>>;
    pub fn live(&self) -> Vec<Arc<HolderLink>>;
}
```

- **Ordering is built in.** A queue's sender exists only inside `Slot::Live`, and
  `publish` creates it pre-filled. So no lock-order rule is left to break.
- **One writer thread per terminal** drains the queue. `Call::frames(speaks)` is the
  only place calls are encoded, and a holder on another major gets only `Close`. As a
  result, the GTK thread no longer writes to sockets, so a stuck holder can't freeze the
  window for its 2 s write timeout.
- `HolderLink` keeps its identity fields and `status`. It loses its stream along with
  `send`, `send_data` and `tell`.
- A close before the holder is ready is queued and replayed. Today it is lost.

### Item 7: Hello first, and the client's major (`hold/serve.rs`)

`greet(input) -> io::Result<Peer>`, with `enum Peer { Speaks, FrozenOnly }`:

- The first frame must be `Hello`.
- A client on another major gets `Welcome` and may send `Close`, but is not joined to
  the output.

### Item 2: push the status on the watch stream

On every change, the phone server writes `Status { status }` to each watcher. On the
window, `remote_boot` loses its re-request loop. The compatibility `Changed` line was
deleted once no old window ran.

### Item 3: drop `waiting`

- Remove `Server.waiting`, its retry loop, `Remote::waiting` and the "other Xenon"
  texts. A start error, port busy included, is reported in a new `RemoteStatus.problem:
  Option<String>`.
- `RemoteStatus` gets `#[serde(default)]`.
- The compatibility `"waiting": false` field was deleted with `Changed`.

### Item 1: fold `xenon_remote` into `xenon_phone`

1. `Mirror` stays in `xenon_term`, because it speaks the private holder protocol. It
   drops `screen` and `history` and gains a read-only accessor: `pub fn read<R>(&self,
   f: impl FnOnce(&Term<impl EventListener>, usize) -> R) -> R`.
2. `mirror_view.rs` and `look.rs` move to `xenon_phone/src/screen.rs`. `xenon_term`
   drops its `xenon_remote` dependency.
3. `xenon_remote/src/*` becomes private modules of `xenon_phone`. Public stays only what
   the window uses: `PhoneLink`, `Pairing`, `RemoteStatus`, `DeviceRow`, `ago`,
   `KEEP_AWAKE_*`, `effective_port` and `normalize_hostname`. The crate is deleted.

### Item 4: key window views by `TermId`

- `terms: HashMap<TermId, Rc<TermView>>`.
- `bodies: HashMap<(WorkspaceId, TabId), Body { widget, term: Option<TermId> }>`.
- `foreign` is deleted, because a foreign tab is a `Body` with a term and no view.
- `view_of(id)` becomes `terms.get(&id)`. A tab's terminal still comes from the model's
  `tab.term`, which stays the only source of truth.

## Synthesis decision

Two sketches were made: A sent requests down the watch stream, and B added a new
`window.sock`.

- **Item 8.** B's shape won, because the server is the caller and finds out whether a
  window is running from the connect alone. A needed ask ids, a capability list and
  asynchronous replies. B's new socket was then dropped, because `ipc.sock` already is
  that socket.
- **Items 5 and 6.** B's queue per terminal is the base, because it builds the ordering
  in and keeps the GTK thread off sockets. Two rules from A were added: calls to unknown
  ids are dropped, and a close made early is queued.
- **Item 4.** A's version: a tab's terminal comes from the model, with no second copy to
  keep in sync.
- **Item 2.** Neither sketch's version was taken. Sending both lines needs no
  negotiation.

## Tradeoffs accepted

- We accept that closing from the phone needs a running, current window. In exchange,
  `workspaces.json` has one writer and the phone close runs the same code as the desktop
  close.
- We accept one writer thread per terminal, in each of the window and the phone server.
  In exchange, ordering holds by construction and the GTK thread never blocks.
- We accept `Mirror::read` exposing alacritty's `Term`. In exchange, nothing has to copy
  the grid.
- We accept that the page learns there's no window only after the confirm, in exchange
  for no presence probe on every listing.

## Alternatives considered

- Requests on the watch stream (sketch A): deeper protocol, with ask ids and capability
  flags, for the same result.
- A new `window.sock` (sketch B): a second request socket into the same window.
- The phone records closes in its own file (rejected by the user): two sources of truth.
- Items 5 and 6 with one lock over two maps: writes would happen under the map lock, and
  ordering would still be a lock-order rule.

## Decisions

- A close ends every terminal of the workspace, on the desktop too, including workspaces
  not opened this session.
- A phone close refuses while an editor in the workspace has unsaved changes. The
  desktop close keeps today's behaviour.

## Next implementation step

`links.rs`, with `Terminals` moved onto it and the public API unchanged.
