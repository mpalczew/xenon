# Hygiene remaining

## Done (pass 3)

| Item | Result |
|------|--------|
| **Legacy `streams/` migrate** | One-shot on `load_registry`; `load_session` no longer reads streams. Dogfood open/closed workspaces migrated to `sessions/`. |
| **`XeroApp` peel** | `app/deferred.rs`: `DeferredUi` + `FocusPane` / `FontPane` (overlay restore + pending command/workspace/palette). |
| **Terminal `view` split** | `view/mod.rs` + Xenon-only `attention.rs` / `paths.rs`. ATTRIBUTION updated. |
| **Settings dropdown selection** | Option rows use `chrome::list_selection` (shared multi-channel paint). Not a full elevated palette shell (settings stays a window). |
| **Icon** | Already `xenon.icns` / Info.plist `xenon` — nothing to do. |

## Still out of scope (not this haul)

| Item | Why |
|------|-----|
| **Crate rename `xero_*` → `xenon_*`** | Optional identity haul (AIPM rename-xenon). Mechanical + large; keep bundle id. |
| **GitHub repo rename** | User action on GitHub Settings. |
| **Homebrew formula** | Public ship later. |
| **Find replace (⌘⌥F)** | Feature backlog. |
| **Settings = full palette shell** | Wrong product shape; settings is a dedicated window. Selection language shared; list UX can iterate later. |
| **Further `XeroApp` peels** | Layout / IDE / git-dirt as owned services when those areas next change. |
| **More terminal extract** | Hover chrome still in `view/mod.rs`; peel when that area is next touched. |

## Verify

- [x] Unit tests on touched crates
- [x] `project install`
