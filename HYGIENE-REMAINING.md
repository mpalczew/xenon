# Hygiene remaining

## Done

| Item | Result |
|------|--------|
| Streams migrate | One-shot on `load_registry` |
| DeferredUi peel | Overlay focus queue |
| Terminal modules | `attention` / `paths` |
| Settings selection paint | Shared chrome, still a window |
| **Crate rename** | `crates/xenon_*`, `XenonApp`, package names |

## Intentionally kept (legacy / TCC)

- Bundle id `dev.xero.xero`
- Data dir migrate `~/.xero` → `~/.xenon`
- Env aliases `XERO_*` → `XENON_*`
- Local codesign identity name `xero-dev` (CDHash / TCC stickiness)

## Still later (not crate hygiene)

| Item | Notes |
|------|--------|
| **Find replace** | Feature never built: in-buffer find (⌘F) ships; **replace** (⌘⌥F / replace all) does not exist. Separate product work. |
| **Homebrew** | Public ship distribution |
| **Screenshots / notarization** | Public packaging polish; see `docs/release.md` |
| **Deeper peels** | See below |

## Deeper peels (what that means)

`XenonApp` still owns almost everything as fields on one type. We only extracted
`DeferredUi` (focus restore + pending palette/command/workspace).

Still mixed on the root type (peel when next touching that area):

1. **Layout** — sidebar/terminal widths, collapse flags, resize handlers
2. **Session surfaces** — terminal stacks, editor stacks, tab menus
3. **Indexes** — file_indexes + index_tasks
4. **Services** — IDE server, git-dirt watcher, attention map
5. **Terminal hover chrome** — still in `view/mod.rs` (On-exit strip, mouse policy)

Not required for correctness; reduces cognitive load when editing those paths.
