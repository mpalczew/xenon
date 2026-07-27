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

- [ ] Rename GitHub `mpalczew/xero` to `mpalczew/xenon` and update the remote.
- [ ] Make the repository public with GPL-3.0 clearly visible.
- [ ] Add a concise README with install instructions, screenshots, and the
      anti-IDE pitch.
- [ ] Add Homebrew distribution.
- [ ] Decide notarization for public binaries.
- [ ] Add short contribution guidance and issue templates that discourage
      low-signal generated submissions.
- [ ] Draft the HN launch around multi-harness freedom, layout, and native feel.

## Rename state

Completed: Xenon app/binary name, `~/.xenon` migration, `XENON_*` environment
variables with legacy aliases, release scripts, crate paths, icon, and launcher
compatibility. The bundle id intentionally remains `dev.xero.xero` so existing
TCC grants survive.

The remaining identity work is the GitHub rename and Homebrew formula. Rename
the existing repository instead of deleting and recreating it so history and
redirects remain intact. Never force-push during that move.
