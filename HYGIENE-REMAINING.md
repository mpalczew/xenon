# Hygiene remaining (pass 2)

After pass 1 (`b74c487`). Scope: leftovers + one WIP already in the tree.

## Land this pass

| Item | Move |
|------|------|
| **Palette keyboard scroll** | WIP already unstaged: `reveal_selected` / `step_selection` on finder, command, task, workspace pickers. Open product gap. Finish + ship. |
| **Shape baseline ratchet** | Drop rows/files under default caps; lower grandfathered numbers to current. |
| **`start_git_dirt_watch` alias** | Fold into `restart_git_dirt_watch` (one entrypoint). |
| **Input handler stubs** | Shared macro/helpers for noop geometry methods on rename + settings (registrar already shared). |

## Keep / later (not this pass)

| Item | Why |
|------|-----|
| **Legacy `streams/` session migrate** | Live data still under `~/.xenon-{a,b}/streams` (+ `~/.xero*`). Delete only after those dirs are empty or one-shot migrated. |
| **`XeroApp` peel** | Real seam design (overlay/focus vs layout vs services). Separate haul. |
| **Terminal `view.rs` split** | Adapted zed surface; extract only Xenon chips when next terminal feature lands. |
| **Crate rename `xero_*`** | AIPM `product/rename-xenon` checklist; not hygiene. |
| **Settings dropdown → palette shell** | Product, not rot. |
| **Find replace, etc.** | Feature backlog. |

## Done when

- Palette arrows keep selection in view; baseline tighter; one git-dirt entrypoint; input stubs not triplicated.
- Tests + `project install`.

## Status (pass 2)

- [x] Palette `reveal_selected` / `step_selection` landed.
- [x] Shape baseline ratcheted.
- [x] `start_git_dirt_watch` folded into `restart_git_dirt_watch`.
- [x] `entity_input_noop_geometry!` for rename, settings, palette query.
- [ ] Install + commit.
