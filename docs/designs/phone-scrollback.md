# Phone scrollback: native scrolling over a server-held line window

Status: approved. Sketches: server-pushed window (A, base) and client-pulled log (B).

## Problem

The phone holds only the visible screen. Each drag asks the server to scroll the mirror
and re-render, so scrolling moves in whole lines at up to 30 fps and needs hand-rolled
touch, wheel and momentum code. The goal is native browser scrolling with about 3
screens of history preloaded above the view, topped up as the user scrolls.

What makes the shape non-obvious:

- **Line identity.** History grows at the bottom while the user reads and is trimmed
  silently at the top once scrollback is full. alacritty does not count lines entering
  history or being trimmed, so stable line numbers have to be made.
- **iOS Safari.** Writing `scrollTop` or inserting content above the view during a drag
  or fling cancels momentum or makes the view jump. Safari has no scroll anchoring.
- **Reflow.** A width change rewraps all history. Alt-screen apps (vim, htop) have no
  history.
- **Callers that can't break.** Each phone attach has its own `Mirror` (`xenon_term`),
  fed by the holder's replay including full history. `FrameEncoder` style interning,
  low-bandwidth mode, the bounded `Outbox` with `force_full` on overflow, and click and
  cursor mapping all stay.
- **Stale pages.** The page is embedded in the binary, but an open page can reconnect
  running old JS.

## Usage (caller's view)

The phone draws a window of lines and scrolls it natively. It tells the server where it
is; the server decides what it holds.

```js
// page.html
case "lines": return renderLines(m);          // replaces "frame"
// on scroll (rAF-throttled, plus scrollend):
send({ t: "scroll", epoch, top, follow });     // top = line number at the viewport top
// "↓ Live": scrollToLive(); send({ t: "scroll", epoch, top, follow: true })
```

```rust
// frames.rs, per tick
let report = view.report();                    // latest phone position, latest wins
let msgs = mirror.read(|term, at| {
    let want = window::plan(at, report, encoder.held(), Depth::of(low_bandwidth));
    encoder.encode(&screen::window(term, at, &want, |n| encoder.holds_history(n, at), &look))
});

// session.rs
ClientMsg::Scroll { epoch, top, follow } => { view.set_report(Report { epoch, top, follow }); mirror.wake() }
ClientMsg::Wheel { rows, col, row } => alt_wheel(&mirror, rows, col, row),   // alt screen only
```

## Shape

**Line numbers (`xenon_term/src/mirror/log.rs`, from B).** Every line gets a `u64`
number, stable within an `epoch`: screen row r is `live + r`, history depth k is `live -
k`, and `oldest = live - history_size` is derived, not stored. The mirror's `Emu` is
created with scrollback `cap + SLACK` (SLACK = 4096). Bytes are advanced in 4 KB slices.
After each slice, history growth is added to `live`, then history is trimmed back to
`cap` with `Grid::update_history` (which drops the oldest lines). Growth per slice is
therefore always measurable. A pure `step(log, before, after, cause)` starts a new epoch
whenever the count can't be known exactly: reset, resize, alt switch, history shrinking,
a CSI 3J or RIS in the slice, or growth past SLACK. Wrong numbers are never served; the
cost is an occasional cache drop. `Numbering { epoch, oldest, live, rows, cols, alt }`
lives under the same mutex as the grid, so reads can't tear. `Mirror::scroll`, `bottom`
and the offset atomic are deleted.

**The window (`xenon_phone/src/window.rs`, from A, pure).** `plan` picks one contiguous
range: the live screen plus 3 screens above the reported top and 1 below (1 and 1 in low
bandwidth), capped at 8 screens. It keeps the current range while it still covers a
comfort zone around the view (hysteresis), so a slow scroll doesn't churn. While
following, the range grows with output and is re-cut at the cap.

**Encoding (`remote/frame.rs`).** `FrameEncoder` becomes `WindowEncoder`. It keeps style
interning and adds `Held` (range plus a `VecDeque` of line hashes) as the single record
of what the phone has. History is immutable within an epoch, so each history line is
rendered once; per tick only the live screen is diffed, as today. A pure scroll sends
only the new lines.

**Wire (`remote/protocol.rs`).**

- `ServerMsg::Lines { epoch, full, cols, rows, alt, oldest, live, top, len, set: [(n,
  spans)], cursor, anchor }` replaces `Frame`. After applying, the phone holds exactly
  `[top, top+len)`, so applying a message twice is harmless.
- `ClientMsg::Scroll { epoch, top, follow }` replaces `Bottom`. `Wheel` gains `col, row`
  and is used only in the alt screen.
- `Ready` gains `build`; a page with a different build reloads itself.

**Page.** Rows are absolutely placed at `(n - origin) * lh` inside one `#screen` whose
height is `(live + rows - origin) * lh`. Adding lines above the view or trimming them
never moves visible content, so `scrollTop` is written only when following the live
edge, on an epoch `anchor`, or when rebasing away dead space above `oldest`. Rebasing
waits for the scroll to be still. Follow is the phone's own state: touching the terminal
turns it off, and it turns back on when scrolling ends at the bottom. The primary screen
uses no custom touch code at all. In the alt screen, the existing pointer-capture drag
sends `wheel`; the server turns it into mouse wheel reports or arrow keys. Cursor and
click map through `live - origin`.

**Hidden behind the surface:** slicing, slack, trimming, epoch rules, preload depth,
hysteresis and line hashes. **Still exposed:** `Term` to `screen.rs`, for colours and
cursor, as today.

## Synthesis decision

- **Base: A.** The server owns the window, so the phone is a renderer with one apply
  function and one report message. Depth, hysteresis and low-bandwidth tuning are pure
  Rust in one place. B's page needed its own cache, chunk tracking, pending retries and
  a `known` correction band, which is more JS and two inputs writing one cache.
- **Taken from B:**
  - Line counting by measured history growth with headroom, plus "unknowable means new
    epoch". A pinned alacritty's `display_offset` as a counter instead; that over-counts
    in apps with scroll regions and depends on a behaviour alacritty doesn't promise.
  - The `Ready.build` self-reload.
- **Rejected:**
  - B's phone-side chunk pulls and render windowing. At most 8 held screens (about 300
    rows) is cheap to keep in the DOM.
  - A's reflow carry scaled by width ratio. Re-anchoring by distance from the live edge,
    as in B, is simpler and just as approximate.

## Tradeoffs accepted

- We accept up to 4096 extra history lines of memory per mirror, and a trim per 4 KB
  slice, in exchange for exact line numbers without forking alacritty.
- We accept a cache drop and a refetch on every reflow, clear, reset or alt exit, in
  exchange for line numbers that are never wrong.
- We accept blank rows for about one round trip when a fling outruns the 3-screen
  preload, in exchange for no phone-side fetch logic.
- We accept dead space above `oldest` while reading during heavy output, until the next
  still moment, in exchange for never moving content under the finger.
- We accept losing momentum in the alt screen: `coast` is deleted, and alt-screen drags
  send plain wheel steps.
- We accept one scroll report per quarter screen of travel. They are tiny messages, and
  the server's hysteresis keeps pushes coarse.

## Alternatives considered

- **Phone-pulled history log (B).** The phone caches aligned chunks and fetches what's
  missing. It lost on interface depth: the page has to know chunking, retries, trims and
  sealing rules, and both the frame stream and pull replies write the phone's cache.
- **Server-scrolled view with smoothing in JS.** Keeps today's protocol, but it can't
  give native momentum, and every step still waits a round trip.
- **Flowed rows with a top spacer.** Sub-pixel rounding between n×lh and n row boxes
  jitters on prepend. Absolute rows don't accumulate error.

## Open questions and risks

- Decided: alt-screen apps lose momentum, and a stale page reloads itself on a build
  mismatch.
- How often does Claude Code clear scrollback (CSI 3J) during normal use? Each clear is
  a new epoch and a re-anchor, which would feel jumpy if it happens often while you
  read. The `fit` journal line could log epoch changes to check.

## Next implementation step

Build `mirror/log.rs` and the sliced, counted advance in `Mirror`, exposing `Numbering`
through `Mirror::read`.
