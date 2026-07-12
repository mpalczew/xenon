# PRODUCT.md

Strategic product context for agents and design work. Visual system lives in
`DESIGN.md` when present; chrome path is Option 2 + 3-lite (AIPM
`projects/xero/design-direction`).

## Register

**product** — design serves the tool. Familiar IDE bones are a feature.
Signature is multi-agent workflow + native speed, not marketing chrome.

## One-liner

Fast native macOS **anti-IDE shell**: gets out of the way so you can live in
many coding agents (any harness) across workspaces. Not a chat panel bolted
onto an editor. Not a vendor-locked agent UI.

## Origin

Started from frustration with available options. Author stack shift: left
Cursor+auto and VS Code+Claude extension for this shell plus mostly Grok, some
Kimi, some Claude. Cursor-like agent layout is close in spirit, but locked to
one harness and heavier. Terminal multiplexers have no stream/project model.
This is the missing layer: multi-stream, any terminal harness, instant feel.

## Category (how we talk about it)

| Layer | Meaning |
|-------|---------|
| **Primary** | Agent shell / multi-agent workbench |
| **Anti-IDE (marketing + product)** | The chrome disappears. Seamless switch Claude Code ↔ Grok ↔ Kimi ↔ anything in a PTY. No harness religion. |
| **IDE (usage)** | "I live here all day." For the author it replaced VS Code. Not a claim of full VS Code feature parity on day one. |
| **Not** | Single-vendor agent UI, cloud collab IDE, or tmux with lipstick |

Most software engineers are becoming prompt engineers (or resisting that). The
trend is accelerating. A daily-driver shell for parallel agent work is the bet.

## Brand name (intent)

The working name **xero** was loved because it sounds like **zero**: nothing.
Subconsciously: anti-IDE, empty chrome, no opinionated agent stack. That
semantic is right even if the spelling must change (Xero accounting collision).

**Settled public name: Xenon.** CLI / app binary: **`xenon`** (not `xe`:
brew + XenServer PATH collision). Metaphor: noble gas, present but inert,
anti-IDE. App: `Xenon.app`. Data: `~/.xenon` (auto-migrate from `~/.xero`).
Env: `XENON_DATA_DIR` / `XENON_SLOT` with legacy `XERO_*` aliases. Bundle id
stays `dev.xero.xero` (TCC). Crate paths still `crates/xero_*`. GitHub repo
rename still open. Checklist: AIPM `projects/xero/rename-xenon`.

Rejected or DQ: xero (accounting), xe CLI, mu (Mu Editor), nyx (Nix verbal +
brew), vexo (vex), soft metaphors (weft/skiff). zeno/wu kept as also-rans only.

## Anti-IDE (product rule, not slogan)

Chrome gets out of the way. Seamless switch between harnesses (Claude Code,
Grok, Kimi, anything in a PTY). No vendor religion. Status exists so agents
are not lost; UI stays quiet. Defaults never imply one model vendor.
Marketing that is true: **the anti-IDE for people who run agents.**

## Wedge → beachhead → north star

| Horizon | Claim |
|---------|--------|
| **Wedge (now)** | People already running several agents / checkouts who are drowning in windows and tabs |
| **Beachhead** | macOS; terminal harnesses (Claude Code and anything that runs in a PTY); multi-workspace |
| **North star (~5y)** | Default place serious people run agent fleets; "everyone who drives agents" is aspiration, not launch copy |

## Users

- **Primary:** super power users juggling multiple agent sessions and workspaces.
- **Secondary (bounce-prevention only):** migrants from Cursor / VS Code / Studio.
  Do not chase full IDE parity for them until primary users are ecstatic.
- **Personas (living):** Alex, Jordan, Night driver; see AIPM
  `projects/xero/personas`.

## Job to be done (JTBD)

**JTBD** = job to be done (the outcome the product is hired for).

> Run several agent-driven workstreams in parallel without losing track of
> which agent is where, what it is waiting on, and what I still own.

How that shows up in the UI: active stream, active terminal/editor, attention
when an agent needs you, dirty buffers. **Today attention/status is incomplete
and has real technical hurdles; it does not work well enough yet.** It is still
central: when it works, it directly helps real daily work.

## Product model

- **Workspace** = project / checkout root.
- **Stream** = unit of work: terminal(s) + editor(s) + layout session.
- **Harness-agnostic shell:** any agent that speaks a terminal. Zero special
  config for a default Claude install. Not a deep multi-provider control plane
  (yet); the vision is the shell, not reimplementing every harness UI.
- **Editor + tree + cmd-p:** enough to live here; terminal is first-class, not
  a drawer.

## Differentiators

1. **Multi-agent, multi-workspace, multi-harness (via PTY)** as the core model
   (streams), not a bolt-on chat panel. Closest analogue: Cursor agent view,
   but you can switch harnesses freely.
2. **Everything feels instant.** Native Rust / GPUI. Typing and switching must
   stay better than Electron-class tools (e.g. Studio feels like a downgrade).
   Protect latency; no decorative motion tax.
3. **macOS-native daily driver** with a clean install path (**Homebrew** for
   public install; stable codesign so TCC grants survive reinstall).

## Competitive frame

| People use today | Gap xero fills |
|------------------|----------------|
| Many terminal tabs / tmux | No stream/workspace model; weak editor; no attention model |
| Cursor / Studio / VS Code AI | Strong single-flow agent UX; often one harness; heavier feel |
| Multiple IDE windows | Context soup; slow context switch |

**Positioning:** the missing layer between a terminal multiplexer and an AI
IDE: a fast native shell for many agent sessions across workspaces.

## Release goals

Visions are directional. Goals are concrete.

- Open source (GPL-3.0).
- Public ship; Hacker News when the wedge is obvious in a few minutes of use.
- **Primary success signal:** substantive issues and PRs from people who care
  enough to engage the codebase. That is the "this is the one" metric.
- **Author proof (already true):** real daily work runs through xero; hard to
  imagine leaving. Keep that bar.
- **Lesser signal:** install counts / "still using after 2 weeks" without
  deeper engagement. Useful, not decisive.
- Context: many OSS projects are drowning in agent-generated noise and have
  little time to approve PRs. Prefer quality engagement over vanity volume.

## Name (release gate)

**Rename ASAP if the collision is material.** Known collisions: Xero accounting
software (large, well-known) and Xero shoes / other consumer marks. Public
launch, search, and HN should not fight that brand. Treat rename as a release
gate, not a soft backlog item. (New name TBD; do not bikeshed in every chat.)

## Brand personality

Cool, dense, calm cockpit. Power-user confident, not playful mascot. Themes are
product surface (trust defaults + personality pack); see AIPM
`projects/xero/themes`.

## Visual direction (chrome)

- **Option 2 now:** selection language, elevation, shared chrome primitives,
  theme-aware attention, consistent icons.
- **Option 3-lite:** agent-cockpit bias (stream attention, terminal status,
  cool density) without a full IA rewrite yet.
- **Default dark:** real elevation (not pure void as the trust default).
- **True Black:** opt-in pure black; selection must stay multi-channel.
- **Tone:** cool (blues / cyans / violets preferred for dogfood).

## Anti-references

- Electron IDEs that feel laggy when typing.
- Chat-sidebar-only AI IDEs that cannot run many parallel harness sessions.
- Decorative AI chrome (gradient text, glassmorphism, hero-metric dashboards).
- Selection that relies on a single background delta.
- Wizard-heavy first-run that blocks power users.
- Claiming full multi-harness "orchestration" when the product is a shell.

## Strategic design principles

1. **Status is never optional.** Active stream, active tab, attention, dirty:
   multi-channel (fill + type and/or indicator). Invest until this is reliable.
2. **Speed is product.** Instant feedback over choreography.
3. **Streams are the unit of work.** Chrome reinforces that hierarchy.
4. **Shell first, IDE bones second.** Tabs / tree / cmd-p stay familiar; the
   multi-stream agent cockpit is the new idea.
5. **Themes must not break structure.** Elevation and selection survive every
   bundled theme, including True Black.
6. **Empty states teach once.** Short power-user copy; no forced tours.
7. **Harness-agnostic by default.** Prefer zero-config terminal integration
   over harness-specific lock-in.

## Accessibility

- High-contrast theme remains available.
- Selection and attention should not rely on color alone where practical.
- Prefer theme tokens over hard-coded hex in chrome.

## Out of scope (for now)

- Full collaborative cloud IDE.
- Replacing every agent harness's own UI.
- Full Option 3 stream-rail redesign until Option 2 chrome system exists.
- Deep multi-provider control plane (shell stays primary).
