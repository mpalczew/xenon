//! Unit tests for markdown preview selection helpers.

use super::{Caret, PreviewSel, selected_markdown};

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
