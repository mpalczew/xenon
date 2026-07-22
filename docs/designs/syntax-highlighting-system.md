# Syntax Highlighting System (Option C)

**Status:** plan reviewed; execute in stages  
**Date:** 2026-07-22  
**Scope:** global highlight pipeline + markdown-first quality + multi-lang injections + priority custom queries

---

## Problem

Editor syntax highlighting is thinner than the stack allows:

1. **Markdown inline is broken** — stock `tree-sitter-md` injection omits `injection.include-children`, so `**bold**`, `*italic*`, links, and `` `code` `` never produce spans.
2. **Paint path drops style** — themes already set `font_weight` / `font_style` (e.g. `emphasis.strong` → 700); `ColoredSpan` only carries color.
3. **Capture vocabulary is narrow** — many grammar captures fall through (`comment.doc`, `string.regex`, `namespace`, `preproc`, …).
4. **Injections unused except markdown (partially)** — HTML, JS, Rust, Svelte, PHP, etc. ship `INJECTIONS_QUERY`; we pass `""` for every non-markdown config.
5. **Custom queries only for proto + powershell** — other langs use stock queries only.

## Goals (done means)

| # | Criterion |
|---|-----------|
| G1 | `.md` source highlights bold, italic, code spans, links, headings, lists, fences (with language injection) |
| G2 | Styled spans apply **color + weight + style** for all languages when the theme provides them |
| G3 | Expanded capture names map to theme keys already present in Xenon themes |
| G4 | High-value languages get stock injections where crates export them |
| G5 | Priority languages get custom or augmented queries where stock is weak / missing |
| G6 | Unit tests cover markdown bold + at least one injection (HTML script or JS regex) + non-regression for existing language smoke tests |
| G7 | `project install` once at end of haul for dogfood |

## Non-goals

- Semantic highlighting / LSP tokens
- Incremental re-highlight (still whole-buffer; fine for v1 sizes)
- Theme redesign (only use existing syntax keys; optional small theme fill-ins if a new capture has no color)
- Perfect Helix/Zed parity for every grammar
- Changing markdown **preview** (already correct via pulldown-cmark)

## Architecture (current → target)

```
path/lang
  → HighlightConfiguration (grammar + highlights + injections)
  → Highlighter events
  → Span { start, end, name }          // capture → theme key
  → StyledSpan { range, color, weight?, style? }  // was ColoredSpan color-only
  → TextRun[] per line
  → shape_line
```

**Markdown special case:** block config + injection callback for `markdown_inline` and fence languages (already); fix injection query.

**Injections general case:** `build_injected(..., injections)` + `highlighter.highlight(..., |name| injection_config(name))` for all configs that have injections — not only markdown.

## Key decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Span type | Rename `ColoredSpan` → `StyledSpan` with optional weight/style | Color-only was the root of “minimal” bold |
| Capture list | Expand `HIGHLIGHT_NAMES` + `theme_key` aliases; rely on tree-sitter-highlight longest-prefix matching | Avoid rewriting every grammar query to our names |
| Markdown injection | Custom `queries/markdown/injections.scm` with `include-children` on `inline` / `pipe_table_cell` | Stock query is wrong for tree-sitter-highlight |
| Multi-lang injections | Prefer crate-exported `INJECTIONS_QUERY` when present; `include_str` fallback only if needed | Less drift from upstream |
| Custom queries | Priority set only (see Stage 5); others stay stock | C without infinite garden |
| Font metrics | Cell width stays monospaced from base font; mixed weight may slightly vary advance | Acceptable; full dual-metric is out of scope |
| Stages | Land each stage green (tests) before next | User asked for staged work |

## Files (expected)

| Path | Role |
|------|------|
| `crates/xenon_editor/src/highlight/mod.rs` | `HIGHLIGHT_NAMES`, `theme_key`, `Span`, markdown path, tests |
| `crates/xenon_editor/src/highlight/grammars.rs` | Configs + injections wiring + `injection_config` |
| `crates/xenon_editor/src/highlight/lang.rs` | Lang enum / name dispatch (only if new injection targets) |
| `crates/xenon_editor/src/element.rs` | `StyledSpan`, `line_runs` / `TextRun` font fields |
| `crates/xenon_editor/src/view/layout.rs` | Resolve full `HighlightStyle` from theme |
| `crates/xenon_editor/queries/markdown/injections.scm` | Fixed md injections |
| `crates/xenon_editor/queries/markdown/highlights.scm` | Block highlights (richer) |
| `crates/xenon_editor/queries/markdown/highlights_inline.scm` | Inline highlights (richer) |
| `crates/xenon_editor/queries/<lang>.scm` | Priority custom highlights (Stage 5) |
| Theme JSON under `crates/xenon_terminal/assets/` | Only if a capture needs a missing key (prefer map to existing) |

## Capture vocabulary (target)

Keep existing names. Add (map via `theme_key` where theme uses different keys):

```
comment.doc          → comment.doc (themes have it)
string.regex         → string.regex
namespace            → namespace (or module fallback in theme_key)
preproc              → preproc
module.builtin       → module
variable.member      → property
variable.special     → variable.special
type.builtin         (already)
punctuation.special  (already)
markup.heading       → title
markup.list          → punctuation.list_marker
markup.link          → link_text
markup.raw           → text.literal
text.strike          → (strikethrough paint if feasible; else color only)
diff.plus / diff.minus (themes have)
```

`theme_key` resolves markdown `text.*` as today. Prefer mapping over inventing new theme keys.

## Stage plan

### Stage 1 — Capture vocabulary

**Deliverable:** more captures survive `configure()` and resolve to theme colors.

- Expand `HIGHLIGHT_NAMES` with the set above (and close cousins present in stock queries).
- Extend `theme_key` for aliases (`markup.heading` → `title`, `text.strong` → `emphasis.strong`, …).
- Test: sample source with a capture that previously dropped (e.g. rust doc comment or JS regex pattern if available without injections) shows non-default span name; markdown title still works.
- Smoke: existing `highlights_common_languages` still passes.

**Exit:** `cargo test -p xenon_editor --lib` green.

### Stage 2 — Styled spans (global paint)

**Deliverable:** weight/style from theme reach `TextRun`.

- Replace `ColoredSpan { color }` with `StyledSpan { color, font_weight, font_style }` (or keep name `ColoredSpan` with extra fields — prefer clear rename).
- `layout.rs`: `style_for_name` → copy `color`, `font_weight`, `font_style` from `HighlightStyle`.
- `line_runs` / `run`: clone base `Font`, apply weight/style when `Some`.
- Do **not** change cell-width calculation (base font).
- Test: pure unit test that `StyledSpan` construction from a known style is non-default weight is hard without theme; at least compile + shape path uses fields. Prefer a small test that `run()` with bold font differs from normal if text system allows; otherwise structural test that layout maps weight through.

**Exit:** tests green; mentally verify `emphasis.strong` path once Stage 3 lands.

### Stage 3 — Markdown fix + richer queries

**Deliverable:** G1.

- Add `queries/markdown/injections.scm`:
  - fenced code → language from info string
  - html_block → html
  - yaml/toml front matter
  - `inline` + `pipe_table_cell` with `markdown_inline` + **`injection.include-children`**
- Add richer `highlights.scm` / `highlights_inline.scm` (stock + improvements: strikethrough if grammar has it, clearer punctuation, list markers).
- Wire `MARKDOWN` / `MARKDOWN_INLINE` to custom files (or block custom + inline stock if inline stock is enough after injection fix).
- Tests:
  - `**bold**` → `emphasis.strong`
  - `*italic*` → `emphasis`
  - `` `code` `` → `text.literal` (or mapped key)
  - `[t](u)` → link captures
  - `# Title` → title
  - fence still injects rust `fn`

**Exit:** markdown tests green.

### Stage 4 — Multi-language injections

**Deliverable:** G4.

Wire `build_injected` with real injection strings where crates export them:

| Lang | Source of injections | Targets we can resolve |
|------|----------------------|-------------------------|
| Markdown | custom (Stage 3) | markdown_inline, fence langs, html, yaml, toml |
| HTML | `tree_sitter_html::INJECTIONS_QUERY` | javascript, css |
| Rust | `tree_sitter_rust::INJECTIONS_QUERY` | rust (macros) |
| JavaScript | `tree_sitter_javascript::INJECTIONS_QUERY` | regex, jsdoc |
| TypeScript/TSX | JS injections + TS highlights (match existing pattern) | regex, jsdoc |
| Svelte | crate query if exportable; else include_str | javascript, typescript, css |
| PHP | if exported | html / php |
| Others with export | enable when `injection_config` can resolve | skip unknown langs quietly |

**General highlight path change:**

```rust
// Today: highlight(config, source) with |_| None
// Target: highlight(config, source, |name| injection_config(name))
```

Use injections for all configs (empty string remains no-op).

**Tests:**

- HTML: `<script>const x = 1</script>` produces a JS keyword span.
- JS: `/ab+c/` produces regex-related span if query injects regex.

**Exit:** new injection tests + full editor lib tests green.

### Stage 5 — Priority custom query garden

**Deliverable:** G5 for high-traffic langs; not every grammar.

**Priority order (stop when diminishing returns):**

1. **Markdown** — done in Stage 3.
2. **Rust** — stock is good; only custom if injections/highlights need locals; prefer stock + injections.
3. **Python / Go / Bash** — stock unless gaps appear in dogfood.
4. **HTML / CSS** — stock + injections usually enough.
5. **YAML / TOML / JSON** — stock.
6. **TypeScript/TSX** — ensure highlight query concatenation + injections stay correct.
7. **Languages with missing upstream constants** — already custom: proto, powershell; fix any new “no highlights” from smoke list.

**Custom query bar:** only add `queries/<lang>.scm` when:

- Upstream ships no `HIGHLIGHTS_QUERY`, or
- Stock query is empty/broken against our capture list, or
- Dogfood shows a clear gap worth 1 focused file.

**Process per language:**

1. Smoke sample in test (already have many).
2. If empty or obviously wrong, pull Helix/nvim query or craft minimal.
3. Attribute provenance in `ATTRIBUTION.md` if copied.

**Exit:** smoke tests still pass; at least one improved language if we find a real gap; otherwise document “stock sufficient after Stages 1–4”.

### Stage 6 — Verify + install

- `cargo test -p xenon_editor --lib`
- `cargo clippy -p xenon_editor -- -D warnings` if project uses it for this crate
- `project install` once
- Offer commit/push (do not commit without user request)

## Testing strategy

| Layer | What |
|-------|------|
| Unit | `spans_for_path` / `spans_for_lang` name assertions on fixtures |
| Regression | Existing multi-lang smoke + env files + sorted spans |
| Manual dogfood | Open README.md, Rust with macros, HTML with script, after install |

## Risks

| Risk | Mitigation |
|------|------------|
| Mixed font weight breaks monospacing slightly | Accept; keep cell width from base font |
| Injection query uses `#eq?` / predicates not supported | Fall back to simplified query; test at wire time |
| Svelte/JS injections pull langs we don’t ship (scss, glimmer) | `injection_config` returns `None` → unhighlighted injection, no crash |
| Capture list too long / order-sensitive | tree-sitter-highlight picks best match by part length; order of `HIGHLIGHT_NAMES` is the highlight index — stable list, append only |
| Nested highlight stack only uses `stack.last()` | Pre-existing; nested styles may lose outer; OK for v1 |

## Stage checklist (execution)

- [x] Stage 1: Capture vocabulary
- [x] Stage 2: Styled spans
- [x] Stage 3: Markdown injection + queries
- [x] Stage 4: Multi-lang injections
- [x] Stage 5: Priority query garden — stock sufficient after 1–4; theme `emphasis` → italic for dogfood
- [x] Stage 6: Verify + `project install`

## Self-review (plan)

| Check | Result |
|-------|--------|
| G1–G7 covered by stages? | Yes: 3→G1, 2→G2, 1→G3, 4→G4, 5→G5, tests in 1–4 →G6, 6→G7 |
| Markdown root cause fixed? | Stage 3 `include-children` |
| Scope creep? | Non-goals list; Stage 5 has a stop bar |
| Dependency order? | 1 before 3 (captures), 2 before dogfood of bold weight, 3 before 4 optional, 4 needs general injection path |
| Placeholders? | No TBDs in critical path; Stage 5 is intentionally adaptive |
| Files named? | Yes |
| Backward compat? | Theme keys unchanged; span type rename is internal to xenon_editor |

**Plan verdict:** ready to execute Stage 1.

## Plan review (2026-07-22)

**Verdict: approve-with-nits** (folded in):

1. Stage 4: injection-query parse failure must **not** drop the whole language config — fall back to empty injections.
2. Stage 1 fixtures must not require injections (use stock captures like rust `///` / comment.doc).
3. Stage 3: TDD — prove bold spans missing, then fix with `include-children`.
4. Stage 5: **stop unless dogfood** — no proactive garden; move TS injection wiring into Stage 4.
5. Optional theme italic on `emphasis` if dogfood wants visible italics (weight already on `emphasis.strong`).
