# Public release checklist

Public packaging is gated on a clear multi-agent daily path, reliable selection
and attention, stable installation, and continued daily dogfood.

## Product gate

- [ ] Selection is unambiguous in One Dark and True Black.
- [ ] Workspaces, terminal tabs, editors, and any PTY harness form a coherent
      daily path.
- [ ] Attention/status is reliable enough to help, or its limits are explicit.
- [x] `project install` uses stable codesigning and preserves TCC grants.
- [x] Xenon remains the author's daily driver.

## Packaging

- [x] Rename GitHub `mpalczew/xero` to `mpalczew/xenon` and update the remote.
- [x] Make the repository public with GPL-3.0 clearly visible (`LICENSE` + README).
- [x] Add a concise README with install instructions and the anti-IDE pitch.
- [x] Add short contribution guidance (`CONTRIBUTING.md`) that discourages
      low-signal generated submissions.
- [ ] Add screenshots to the README.
- [ ] Add Homebrew distribution.
- [ ] Decide notarization for public binaries.
- [ ] Add issue templates (optional; CONTRIBUTING covers the anti-noise bar).
- [ ] Draft the HN launch around multi-harness freedom, layout, and native feel.

## Rename state

Completed: Xenon app/binary name, `~/.xenon` migration, `XENON_*` environment
variables with legacy aliases, release scripts, crate paths, icon, launcher
compatibility, and GitHub `mpalczew/xenon` (public). The bundle id intentionally
remains `dev.xero.xero` so existing TCC grants survive.

Remaining identity / distribution work: Homebrew formula and notarization.
Never force-push.
