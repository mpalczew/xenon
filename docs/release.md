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

- [x] Make the repository public with GPL-3.0 clearly visible (`LICENSE` + README).
- [x] Add a concise README with install instructions and the anti-IDE pitch.
- [x] Add short contribution guidance (`CONTRIBUTING.md`) that discourages
      low-signal generated submissions.
- [ ] Add screenshots to the README.
- [ ] Add Homebrew distribution.
- [ ] Decide notarization for public binaries.
- [ ] Add issue templates (optional; CONTRIBUTING covers the anti-noise bar).
- [ ] Draft the HN launch around multi-harness freedom, layout, and native feel.

Remaining identity / distribution work: Homebrew formula and notarization.
Never force-push.
