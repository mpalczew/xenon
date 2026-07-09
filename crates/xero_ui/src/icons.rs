use std::borrow::Cow;

use gpui::{App, IntoElement, ParentElement, Pixels, Styled, div, px};
use lucide_icons::{Icon, LUCIDE_FONT_BYTES};

const LUCIDE_FONT_FAMILY: &str = "Lucide";

pub(crate) fn load_icon_font(cx: &mut App) {
    if let Err(error) = cx
        .text_system()
        .add_fonts(vec![Cow::Borrowed(LUCIDE_FONT_BYTES)])
    {
        log::error!("failed to load Lucide icon font: {error}");
    }
}

pub(crate) fn icon(icon: Icon, size: Pixels) -> impl IntoElement {
    div()
        .font_family(LUCIDE_FONT_FAMILY)
        .text_size(size)
        .line_height(size)
        .child(char::from(icon).to_string())
}

pub(crate) fn preview_icon(previewing: bool) -> impl IntoElement {
    let icon = if previewing {
        Icon::FileText
    } else {
        Icon::Eye
    };
    self::icon(icon, px(15.))
}
