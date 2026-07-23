//! Unit tests for markdown preview selection helpers.

use super::{
    Caret, PreviewSel, SelectMode, line_range_at, select_all_carets, selected_markdown,
    word_range_at,
};
use gpui::SharedString;

fn blocks(parts: &[&str]) -> Vec<SharedString> {
    parts.iter().map(|s| SharedString::from(*s)).collect()
}

#[test]
fn selected_markdown_single_block_is_source_slice() {
    let source = "# Hello **world**\n\nPara.\n";
    // block 0 = heading 0..18, block 1 = para 20..26 (approx)
    let ranges = vec![0..18, 20..26];
    let sel = PreviewSel {
        anchor: Caret {
            block: 0,
            offset: 0,
        },
        head: Caret {
            block: 0,
            offset: 3,
        },
        ..Default::default()
    };
    // Partial highlight still copies whole heading markdown.
    assert_eq!(
        selected_markdown(source, &ranges, &sel),
        "# Hello **world**\n"
    );
}

#[test]
fn selected_markdown_multi_block_contiguous_source() {
    let source = "# Hello **world**\n\nPara with `code`.\n";
    let ranges = vec![0..18, 19..37];
    let sel = PreviewSel {
        anchor: Caret {
            block: 0,
            offset: 0,
        },
        head: Caret {
            block: 1,
            offset: 4,
        },
        ..Default::default()
    };
    assert_eq!(
        selected_markdown(source, &ranges, &sel),
        "# Hello **world**\n\nPara with `code`.\n"
    );
}

#[test]
fn selected_markdown_reversed_drag() {
    let source = "one\n\ntwo\n";
    let ranges = vec![0..4, 5..9];
    let sel = PreviewSel {
        anchor: Caret {
            block: 1,
            offset: 3,
        },
        head: Caret {
            block: 0,
            offset: 1,
        },
        ..Default::default()
    };
    assert_eq!(selected_markdown(source, &ranges, &sel), "one\n\ntwo\n");
}

#[test]
fn empty_selection_is_empty_string() {
    let source = "x";
    let ranges: Vec<_> = std::iter::once(0..1).collect();
    let sel = PreviewSel {
        anchor: Caret {
            block: 0,
            offset: 1,
        },
        head: Caret {
            block: 0,
            offset: 1,
        },
        ..Default::default()
    };
    assert!(selected_markdown(source, &ranges, &sel).is_empty());
}

#[test]
fn select_all_mode_copies_full_source() {
    let source = "# A\n\nB\n";
    let ranges = vec![0..4, 5..7];
    let sel = PreviewSel {
        anchor: Caret {
            block: 0,
            offset: 0,
        },
        head: Caret {
            block: 1,
            offset: 1,
        },
        mode: SelectMode::All,
        ..Default::default()
    };
    assert_eq!(selected_markdown(source, &ranges, &sel), source);
}

#[test]
fn word_range_ident() {
    assert_eq!(word_range_at("foo_bar baz", 2), 0..7);
    assert_eq!(word_range_at("foo_bar baz", 8), 8..11);
}

#[test]
fn line_range_multiline() {
    let t = "ab\ncd\nef";
    assert_eq!(line_range_at(t, 1), 0..2);
    assert_eq!(line_range_at(t, 3), 3..5);
    assert_eq!(line_range_at(t, 7), 6..8);
}

#[test]
fn local_range_middle_block_full() {
    let sel = PreviewSel {
        anchor: Caret {
            block: 0,
            offset: 1,
        },
        head: Caret {
            block: 2,
            offset: 2,
        },
        ..Default::default()
    };
    assert_eq!(sel.local_range(1, 10), Some(0..10));
    assert_eq!(sel.local_range(0, 5), Some(1..5));
    assert_eq!(sel.local_range(2, 5), Some(0..2));
    assert_eq!(sel.local_range(3, 5), None);
}

#[test]
fn select_all_carets_span_doc() {
    let b = blocks(&["a", "bb"]);
    let (a, h) = select_all_carets(&b).unwrap();
    assert_eq!(a.offset, 0);
    assert_eq!(h.block, 1);
    assert_eq!(h.offset, 2);
}
