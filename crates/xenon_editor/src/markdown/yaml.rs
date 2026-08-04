//! YAML frontmatter parse + preview panel chrome.

use std::ops::Range;

use gpui::{
    AnyElement, Entity, FontWeight, Hsla, IntoElement, ParentElement, Pixels, SharedString, Styled,
    div, px,
};

use super::selectable::SelectableBlock;
use super::state::PreviewState;

/// Paint tokens for frontmatter panel chrome + selectable fields.
pub(super) struct MetaPaint {
    pub rule: Hsla,
    pub code_bg: Hsla,
    pub muted: Hsla,
    pub base: Pixels,
    pub mono: String,
    pub host: Entity<PreviewState>,
    pub selection: Hsla,
}

/// Render frontmatter body; return plain texts + source ranges in document order.
pub(super) fn render_metadata(
    body: &str,
    source_range: Range<usize>,
    paint: &MetaPaint,
    block_base: usize,
) -> (AnyElement, Vec<SharedString>, Vec<Range<usize>>) {
    if let Some(pairs) = split_simple_yaml_pairs(body) {
        pairs_panel(pairs, source_range, paint, block_base)
    } else {
        raw_panel(body, source_range, paint, block_base)
    }
}

fn pairs_panel(
    pairs: Vec<(String, String)>,
    source_range: Range<usize>,
    paint: &MetaPaint,
    mut block_ix: usize,
) -> (AnyElement, Vec<SharedString>, Vec<Range<usize>>) {
    let mut plains = Vec::new();
    let mut ranges = Vec::new();
    let mut panel = div()
        .my_3()
        .w_full()
        .min_w_0()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(paint.rule)
        .bg(paint.code_bg)
        .flex()
        .flex_col()
        .gap_2();
    for (key, value) in pairs {
        // Keys/values share the frontmatter source range (block-granular copy).
        let key_s = SharedString::from(key);
        let val_s = SharedString::from(value);
        plains.push(key_s.clone());
        ranges.push(source_range.clone());
        plains.push(val_s.clone());
        ranges.push(source_range.clone());
        let key_el = SelectableBlock::new(
            block_ix,
            key_s,
            Vec::new(),
            paint.host.clone(),
            paint.selection,
        );
        block_ix += 1;
        let value_el = SelectableBlock::new(
            block_ix,
            val_s,
            Vec::new(),
            paint.host.clone(),
            paint.selection,
        );
        block_ix += 1;
        panel = panel.child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(f32::from(paint.base) * 0.85))
                        .font_weight(FontWeight::BOLD)
                        .text_color(paint.muted)
                        .child(key_el),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .whitespace_normal()
                        .text_size(paint.base)
                        .child(value_el),
                ),
        );
    }
    (panel.into_any_element(), plains, ranges)
}

fn raw_panel(
    body: &str,
    source_range: Range<usize>,
    paint: &MetaPaint,
    block_ix: usize,
) -> (AnyElement, Vec<SharedString>, Vec<Range<usize>>) {
    let text = SharedString::from(body.to_string());
    let child = SelectableBlock::new(
        block_ix,
        text.clone(),
        Vec::new(),
        paint.host.clone(),
        paint.selection,
    );
    let element = div()
        .my_3()
        .w_full()
        .min_w_0()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(paint.rule)
        .bg(paint.code_bg)
        .font_family(paint.mono.clone())
        .text_size(px(f32::from(paint.base) * 0.9))
        .whitespace_normal()
        .child(child)
        .into_any_element();
    (element, vec![text], vec![source_range])
}

/// Parse simple `key: value` YAML (including indented multi-line values).
/// Returns `None` when the body is not a flat list of pairs.
pub(super) fn split_simple_yaml_pairs(body: &str) -> Option<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();

    for line in body.lines() {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            current_key.as_ref()?;
            let piece = line.trim();
            if piece.is_empty() {
                continue;
            }
            if !current_val.is_empty() {
                current_val.push(' ');
            }
            current_val.push_str(piece);
            continue;
        }

        if let Some(key) = current_key.take() {
            let value = current_val.trim().to_string();
            if value.is_empty() {
                return None;
            }
            pairs.push((key, value));
            current_val.clear();
        }

        let (key, rest) = line.split_once(':')?;
        let key = key.trim();
        if key.is_empty() || key.chars().any(char::is_whitespace) {
            return None;
        }
        current_key = Some(key.to_string());

        let rest = rest.trim();
        // Drop common block/fold markers; keep any remaining same-line value.
        let rest = rest
            .strip_prefix(">-")
            .or_else(|| rest.strip_prefix("|-"))
            .or_else(|| rest.strip_prefix('>'))
            .or_else(|| rest.strip_prefix('|'))
            .unwrap_or(rest)
            .trim();
        if !rest.is_empty() {
            current_val = rest.to_string();
        }
    }

    if let Some(key) = current_key {
        let value = current_val.trim().to_string();
        if value.is_empty() {
            return None;
        }
        pairs.push((key, value));
    }

    if pairs.is_empty() { None } else { Some(pairs) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_pairs_single_line() {
        let pairs = split_simple_yaml_pairs("name: brainstorming\ndescription: Open space.\n")
            .expect("pairs");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("name".into(), "brainstorming".into()));
        assert_eq!(pairs[1], ("description".into(), "Open space.".into()));
    }

    #[test]
    fn yaml_pairs_multiline_description() {
        let body =
            "name: brainstorming\ndescription:\n  Open the problem space before\n  narrowing it.\n";
        let pairs = split_simple_yaml_pairs(body).expect("pairs");
        assert_eq!(pairs[0].0, "name");
        assert_eq!(pairs[1].0, "description");
        assert!(pairs[1].1.contains("Open the problem space"));
        assert!(pairs[1].1.contains("narrowing it."));
    }

    #[test]
    fn yaml_pairs_folded_marker() {
        let body = "name: hygiene\ndescription: >-\n  Codebase hygiene scan.\n";
        let pairs = split_simple_yaml_pairs(body).expect("pairs");
        assert_eq!(pairs[0].1, "hygiene");
        assert_eq!(pairs[1].1, "Codebase hygiene scan.");
    }

    #[test]
    fn yaml_pairs_rejects_orphan_indent() {
        assert!(split_simple_yaml_pairs("  just indented\n").is_none());
    }

    #[test]
    fn yaml_pairs_rejects_empty_value() {
        assert!(split_simple_yaml_pairs("name:\n").is_none());
    }
}
