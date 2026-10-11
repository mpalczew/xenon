//! Bundled faces loaded into GPUI at startup.
//!
//! Lilex and IBM Plex Sans: see ATTRIBUTION.md (OFL-1.1, via zed assets).

use std::borrow::Cow;

use gpui::App;

/// Register bundled faces so editor, terminal, and UI chrome resolve.
///
/// Linux maps `.SystemUIFont` to IBM Plex Sans. Without these files the
/// chrome falls through the fallback stack onto Lilex.
pub fn load_embedded_fonts(cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../fonts/lilex/Lilex-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../fonts/lilex/Lilex-Bold.ttf")),
        Cow::Borrowed(include_bytes!("../fonts/lilex/Lilex-Italic.ttf")),
        Cow::Borrowed(include_bytes!("../fonts/lilex/Lilex-BoldItalic.ttf")),
        Cow::Borrowed(include_bytes!(
            "../fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../fonts/ibm-plex-sans/IBMPlexSans-Italic.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../fonts/ibm-plex-sans/IBMPlexSans-SemiBoldItalic.ttf"
        )),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::error!("failed to load bundled fonts: {error}");
    }
}
