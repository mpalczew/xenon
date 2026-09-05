//! Bundled faces loaded into GPUI at startup.
//!
//! Lilex: see ATTRIBUTION.md (OFL-1.1, via zed assets/fonts/lilex).

use std::borrow::Cow;

use gpui::App;

/// Register Lilex (OFL) so editor/terminal pickers and defaults resolve.
pub fn load_embedded_fonts(cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../fonts/lilex/Lilex-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../fonts/lilex/Lilex-Bold.ttf")),
        Cow::Borrowed(include_bytes!("../fonts/lilex/Lilex-Italic.ttf")),
        Cow::Borrowed(include_bytes!("../fonts/lilex/Lilex-BoldItalic.ttf")),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::error!("failed to load Lilex: {error}");
    }
}
