//! Editable Markdown "prose" mode: a projection of the one source buffer.
//! Model parses blocks, display maps source to shown text, layout shapes it.

pub mod display;
pub mod edit;
pub mod layout;
pub mod model;
pub mod paint;
pub mod projection;
pub mod style;

#[cfg(test)]
mod tests {
    use ropey::Rope;

    use super::edit;
    use super::projection::Projection;

    const DOC: &str = "<!-- a -->\n\n# T\n\nPara one\nwraps here.  \nhard\n\n> q one\n> q two\n\n- [ ] a\n  - b\n1. x\n\n```rs\nlet a = 1;\n```\n\n| h | i |\n| - | - |\n| 1 | 2 |\n";

    /// Every source offset that may hold the caret maps to a drawn place and
    /// back to itself: the one char/byte boundary is exact.
    #[test]
    fn caret_offsets_round_trip_through_the_display() {
        for hide in [false, true] {
            let rope = Rope::from_str(DOC);
            let proj = Projection::build(&rope, true, hide);
            for pos in 0..=rope.len_chars() {
                if !proj.valid(pos) {
                    continue;
                }
                let (bi, at) = proj.locate(pos);
                let back = proj.displays[bi].src_at(proj.block(bi), at);
                let (bj, at2) = proj.locate(back);
                assert_eq!((bi, at), (bj, at2), "hide={hide} pos={pos}");
            }
        }
    }

    #[test]
    fn enter_continues_lists_and_ends_them() {
        let rope = Rope::from_str("- [x] done\n1. one\n- \n");
        let proj = Projection::build(&rope, true, false);
        let at = |p| edit::enter(&rope, &proj, p, false);
        assert_eq!(at(10).text, "\n- [ ] ");
        assert_eq!(at(17).text, "\n2. ");
        let empty = at(20);
        assert_eq!((empty.range, empty.text.as_str()), (18..20, ""));
    }

    #[test]
    fn unicode_offsets_stay_chars() {
        let rope = Rope::from_str("é **b** ñ\n");
        let proj = Projection::build(&rope, true, false);
        for pos in 0..=rope.len_chars() {
            let (bi, at) = proj.locate(pos);
            let back = proj.displays[bi].src_at(proj.block(bi), at);
            assert!(back <= rope.len_chars());
        }
    }
}
