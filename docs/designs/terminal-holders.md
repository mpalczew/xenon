# Terminal holders

Each terminal runs in its own small process, a holder, started from whatever
`xenon-term` binary is installed at that moment. Installing a new build changes only
terminals opened afterwards. Running holders keep their old code until their terminal
closes, the way sshd sessions keep running across an sshd upgrade. This replaced an
earlier single terminal service, whose emulator, replay, size claims and status code the
holder kept.

## Problem

With one service, upgrading the service ends every terminal. The user wants upgrades to
affect only new terminals, with several builds running at once and the window showing
which terminals are on an older build. Grounding brought up these constraints:

- **Version skew is now permanent.** A new window talks to older holders for as long as
  any of them lives, possibly weeks. The contract between builds must be small, and its
  compatibility code must be deletable.
- **Phones must work with no window open, and see every build's terminals.**
- **The window needs a discoverable menu:** Close Window, Quit and End Terminals, and
  Preferences, plus `xenon quit --end` for shells.
- **Least code; use systemd.** `systemd-run` transient units, slices and `Type=notify`.

## Usage (caller's view)

```rust
use xenon_term::{Terminals, Role, Spawn, Event};

// Window startup. There is no connect step that can fail: the runtime directory is the
// list of terminals.
let terms = Terminals::start(Role::Desk);
terms.set_base_env(child_env());           // phone-made terminals get the desktop env

while let Ok(ev) = terms.events().recv().await {
    match ev {
        Event::Ready { id, info } => app.adopt(info.workspace, id), // window- or phone-made
        Event::Output { id, bytes } => app.view(id).feed(&bytes),    // first after attach: replay
        Event::Status { id, status } => app.term_status(id, status), // dots, title, exit
        Event::Size { id, size } => app.view(id).follow(size),
        Event::Closed { id } => app.term_closed(id),                 // holder gone, any reason
        Event::Builds => app.paint_build_marks(),                    // install replaced the binary
    }
}

let id = terms.open(ws, Spawn::shell(&root));   // id at once; the holder starts off-thread
term.connect_commit(move |_, text, _| terms.input(id, text.as_bytes()));
terms.attach(id, view.size());
terms.seen(id);
terms.close(id);                                // one terminal

// Header bar: worked out when read, never stored.
let older = terms.list().into_iter().filter(|t| t.build.older).count();
xenon_term::end_all()?;                         // Quit and End Terminals: every build
```

```rust
// xenon_phone (its own process): the same API, Role::Phone.
let terms = Terminals::start(Role::Phone);
let mut mirror = terms.watch(id, Claim::phone(size, "Pixel"))?; // its own emulator copy and scroll
mirror.scroll(-5);                                              // never moves the computer
```

## Shape

### Processes

| Process                         | Started by                                      | Unit                                                                                                    | Lives until            | Build                               |
| ------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ---------------------- | ----------------------------------- |
| holder `xenon-term hold <id>`   | `Terminals::open`, through `systemd-run --user` | `xenon-term-<id>.service` in `xenon-term.slice`, `Type=notify`, `KillSignal=SIGHUP`, `TimeoutStopSec=5` | its terminal is closed | the build installed when it started |
| phone server `xenon-term phone` | login, and `xenon-phone.socket` on demand       | `xenon-phone.service`                                                                                   | logout                 | newest: install runs `try-restart`  |
| window `xenon-bin`              | `~/.local/bin/xenon`                            | app scope                                                                                               | the user quits         | newest, from its next start         |

### Discovery: the runtime directory is the registry

```
$XDG_RUNTIME_DIR/xenon/term/<TermId>.sock   a ready holder (renamed into place once its child runs)
$XDG_RUNTIME_DIR/xenon/term/<TermId>.spec   spawn request, mode 0600; the holder reads and deletes it
$XDG_RUNTIME_DIR/xenon/base-env             the window's env for phone-made terminals; the window is its only writer
$XDG_RUNTIME_DIR/xenon/phone.sock           phone server control: pairing, status, settings
```

No process keeps a list of terminals. A client reads the directory, watches it with
inotify, and holds one connection per holder. A connection that ends means the terminal
is gone. A socket that refuses connections belongs to a crashed holder and is deleted by
whoever finds it.

### Holder (today's `Term`, alone in a process)

`Holder { id, workspace, build, pid, master, state: Mutex<State> }`, where `State { emu,
status, claims, viewers, closed_at }`. Today's `desk` and `watchers` fold into one
`viewers` list, where at most one viewer (the window's VTE) answers terminal queries.
Output, attach-with-replay, the 4 MiB drop and repaint, size claims and status all work
as they do in the built service. When the child exits, the holder stays alive so the tab
can show the exit code and the final screen. It exits after `Close`, a unit stop, or a
grace timer.

### The contract between builds

One frame type: `u32 length | kind u8 | body`, where `kind` is control (JSON) or data
(raw bytes for output and input). JSON uses tolerant serde: unknown fields are ignored,
unknown messages are skipped, and new fields are `#[serde(default)]`.

```rust
enum ToHolder   { Hello{major,minor} /*FROZEN*/, Close /*FROZEN*/, View{answers},
                  Claim{size, label: Option<String>}, Seen }      // + data frames: input
enum FromHolder { Welcome{major,minor,build,workspace,pid} /*FROZEN*/, Status(TermStatus),
                  Size(Size), Screen /* reset, then replay as data */ } // + data frames: output
```

- **Additions are free.** A new field or message goes in under the same major version.
  The minor number tells a client which features an older holder has.
- **A change is a new major.** Its decoder for the old version sits alone in
  `proto/v<N>.rs`, headed "delete when no holder of major N lives". The header-bar
  indicator tells you when that is: zero older terminals.
- **Frozen in every major:** `Hello`, `Welcome` and `Close`. So any window can always
  list, mark and close any holder. A holder whose major the window can't read shows
  "This terminal needs a newer Xenon window".
- **The other cross-build contracts are tiny:** the spec file (JSON, five fields), the
  directory layout, and the unit and slice names.

### "Older build"

A holder is older when `/proc/<holder pid>/exe` is a different inode from the installed
binary. The holder's pid comes from `SO_PEERCRED` on its socket. Install replaces the
binary by rename, so this needs no stamps and no list of builds. `Terminals` watches the
app folder and sends `Event::Builds` when the binary changes.

- **Header bar:** a button, shown only when some terminal is older: "3 on an older
  build". Its popover lists those terminals by workspace and tab title, with each one's
  build. Clicking a row selects that tab. There's no bulk restart, because restarting a
  terminal ends its agent.
- **Per tab:** a small mark, with the tooltip "Running build 3ad2c53. New terminals use
  9d5bfb9."

### Header-bar menu

Close Window (terminals keep running), Quit and End Terminals (asks first and says how
many terminals are running across all builds), and Preferences. The first time you close
the window, a toast says the terminals keep running. Quit and End Terminals and `xenon
quit --end` both call `end_all()`, which runs `systemctl --user stop xenon-term.slice`.
Each holder gets a hangup, then SIGKILL after 5 seconds, so hung holders and holders of
unreadable versions end too. Closing one tab sends `Close`.

### Settings: window and services kept apart

- **`settings.json`** is the window's: fonts, themes, editor, sidebar, the window, and
  terminal auto-close.
- **`services.json`** is new and belongs to the background processes. It holds the phone
  remote's enabled, port, hostname and keep-awake, and terminal scrollback lines. It is
  a `ServiceSettings` type in `xenon_store` with tolerant serde, because older holders
  read it too.
- **Both are edited in Preferences,** and the window is the only writer of both files.
  Services never write settings. Today the service writes `remote_enabled`; that moves
  to the window.
- **The phone server reloads** `services.json` when it changes (inotify).
- **A holder reads it once at start,** so a change applies to new terminals, like a new
  build.

### Phone server

Today's `phone/` module moves to its own crate, `xenon_phone`, built on the public
`Terminals` API, so it can't reach holder internals:

- **Listings and dots:** from holder `Status`; workspace names from the registry file,
  as today.
- **Watching:** a phone opens its own viewer connection and keeps a `Mirror` (the same
  `Emu`), so scroll and frames are per phone. The holder's existing claim merge decides
  the size.
- **Tasks:** follow `Status.exit` by `TermId`.
- **The window's phone settings:** over `phone.sock`.
- **Restarts:** `xenon-phone.service` has `Restart=on-failure` with `RestartSec=2s`, so
  a crash brings it back by itself. Install runs `systemctl --user try-restart
  xenon-phone.service`, or starts it when it is not running. The phone page in
  Preferences has a "Restart Phone Server" button that runs `systemctl --user restart
  xenon-phone.service` and says that phones reconnect by themselves.

### Crate changes

- **`xenon_term`:** `term.rs`, `reader.rs`, `pty.rs`, the `emu*.rs` files, `status.rs`
  and `claims.rs` move under `hold/`. `client.rs` becomes `terminals.rs`. The new pieces
  are `proto/`, `start.rs` (systemd-run plus the spec file), `dir.rs` (discovery) and
  `builds.rs`.
- **Deleted:** `terms.rs`, `hub.rs`, `service.rs`, `codec.rs`, `dec.rs`, and the single
  socket and service units.
- **`phone/`** moves to `xenon_phone`.

## Synthesis decision

The base is **D** (a holder per terminal). It is the only shape where a bad build can
reach just the terminals opened on it, which is what makes the ssh model worth having. A
holder crash also ends only its own terminal. Its interface is today's `Client`,
reshaped.

- **From E:** JSON control frames with tolerant serde, in place of the hand-written
  binary codec, as D's own open question suggested. Also E's rule for deleting old
  compatibility code once nothing that old is running.
- **From F:** nothing structural. Its live handover moves every agent onto new code at
  once, which is the opposite of what was asked. It costs a pause and a repaint on every
  upgrade, and needs a frozen snapshot format.
- **Rejected E as the base:** it means routing, draining, `not_current` retries and a
  timer, with no crash isolation, to save processes we can afford.

## Tradeoffs accepted

- **Memory:** we accept 2 to 4 MB more per terminal (roughly 100 to 150 MB at 40
  terminals) in exchange for isolation per terminal and per build.
- **Compatibility:** we accept a frozen core of three messages and additions-only growth
  in exchange for never ending agents on upgrade.
- **Opening a terminal:** we accept 30 to 80 ms (systemd-run, off the main thread) in
  exchange for each terminal being its own systemd unit, visible in `systemctl` and the
  journal.
- **Phone task chains:** we accept that a phone-started chain of dependent tasks stops
  if the phone server restarts, in exchange for upgrading the phone server freely.

## Alternatives considered

- **E, one service per build, with old ones draining:** fewer processes, but more moving
  parts (routing, a retry when a build is no longer current, a drain timer) and a crash
  still takes a whole build's terminals.
- **F, a live upgrade through the systemd fd store:** one version always, so there's no
  indicator. But every upgrade pauses and repaints every agent, loses the scroll region,
  tab stops and character sets, depends on systemd 254 and kernel 6.15, and a subtle bug
  in a new build reaches all agents at once.

## Open questions and risks

- **Risk:** the frozen core and additions-only rule have to hold for weeks of builds. Is
  it acceptable to try the protocol for a few days before relying on it across upgrades?
- **Decided:** the per-tab mark always shows on older terminals, kept subtle.
- **Decided:** start running on the new build as soon as it is installed; the protocol
  proves itself in use.

## Next implementation step

Move the holder out of `Terms` into `xenon-term hold`, started with `systemd-run`, with
the window reading the runtime directory. Then the phone process, then the header bar.

## As built

Differences from the sketch above, all additive:

- **`xenon-term` lives in the `xenon_phone` crate.** The phone server uses only
  `xenon_term`'s public API, and the one binary has to carry `hold`, `phone`, `end-all`
  and `version`; a binary in `xenon_term` could not depend on `xenon_phone`.
- **`PhoneLink` lives in `xenon_phone`,** over `phone.sock` (one JSON object per line).
  The window writes `services.json` and then calls `Status`, which makes the server
  re-read it; the file watch covers hand edits. A `Watch` call keeps a connection open
  for `Changed` pushes, and `Dark` tells the server the window's theme.
- **`watch(id, label)` takes the phone's label, not a size,** because the size arrives
  later from the phone (`Mirror::fit`).
- **`Welcome` carries `cwd`** (added, with a default), so listings need no second call.
- **Attach, resize and typing that happen before the holder is connected** wait and are
  applied when it is, so a new tab does not lose its first keys.
- **Private runs:** `XENON_RUNTIME_DIR` replaces `$XDG_RUNTIME_DIR/xenon`, and
  `XENON_TERM_SLICE` names another slice. Both, and `XENON_DATA_DIR`, are passed on to
  holders through `systemd-run --setenv`.
- **First close:** the toast appears and the window closes about two seconds later, so
  the toast can be read. It is shown once (`close_hint_shown` in `settings.json`).
