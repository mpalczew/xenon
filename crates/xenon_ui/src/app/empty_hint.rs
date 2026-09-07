//! Empty editor pane hint: shrink-to-fit, wrap only at the floor size.

use gpui::{
    AnyElement, Bounds, Hsla, IntoElement, ParentElement, Pixels, ShapedLine, SharedString, Styled,
    TextAlign, TextRun, Window, WrappedLine, canvas, div, point, px,
};

/// Empty editor: shrink font to fit on one line, wrap only at the floor size.
/// Small L/R padding; painted via canvas so wrap width is the real pane width
/// (avoids flex min-width collapse that stacked characters vertically).
pub(super) fn empty_editor_hint(color: Hsla) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_hidden()
        .px_1()
        .child(
            canvas(
                move |bounds, window, _cx| fit_hint(bounds, color, window),
                paint_fitted_hint,
            )
            .size_full(),
        )
        .into_any_element()
}

fn fit_hint(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) -> FittedHint {
    const LABEL: &str = "Open file (⌘P) · New terminal (⌘N) · File tree (⌘E)";
    const SIZES: [f32; 6] = [16., 14., 12., 11., 10., 9.];
    let avail = bounds.size.width;
    if avail <= px(0.) {
        return FittedHint::Empty;
    }
    let font = window.text_style().font();
    let text = SharedString::from(LABEL);
    let run = TextRun {
        len: text.len(),
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    for &size in &SIZES[..SIZES.len() - 1] {
        let font_size = px(size);
        let line = window.text_system().shape_line(
            text.clone(),
            font_size,
            std::slice::from_ref(&run),
            None,
        );
        if line.width() <= avail {
            return FittedHint::Single {
                line: Box::new(line),
                size,
            };
        }
    }
    let size = SIZES[SIZES.len() - 1];
    let font_size = px(size);
    match window.text_system().shape_text(
        text,
        font_size,
        std::slice::from_ref(&run),
        Some(avail),
        None,
    ) {
        Ok(lines) => FittedHint::Wrapped {
            lines: lines.into_vec(),
            size,
        },
        Err(_) => FittedHint::Empty,
    }
}

fn paint_fitted_hint(
    bounds: Bounds<Pixels>,
    fitted: FittedHint,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    const LINE_FACTOR: f32 = 1.2;
    let line_height = |size: f32| px(size * LINE_FACTOR);
    match fitted {
        FittedHint::Empty => {}
        FittedHint::Single { line, size } => {
            let lh = line_height(size);
            let x = bounds.origin.x + ((bounds.size.width - line.width()) / 2.).max(px(0.));
            let y = bounds.origin.y + ((bounds.size.height - lh) / 2.).max(px(0.));
            let _ = line.paint(point(x, y), lh, TextAlign::Left, None, window, cx);
        }
        FittedHint::Wrapped { lines, size } => {
            let lh = line_height(size);
            let mut total_h = px(0.);
            for line in &lines {
                total_h += line.size(lh).height;
            }
            let mut y = bounds.origin.y + ((bounds.size.height - total_h) / 2.).max(px(0.));
            for line in &lines {
                let _ = line.paint(
                    point(bounds.origin.x, y),
                    lh,
                    TextAlign::Center,
                    Some(bounds),
                    window,
                    cx,
                );
                y += line.size(lh).height;
            }
        }
    }
}

enum FittedHint {
    Empty,
    Single { line: Box<ShapedLine>, size: f32 },
    Wrapped { lines: Vec<WrappedLine>, size: f32 },
}
