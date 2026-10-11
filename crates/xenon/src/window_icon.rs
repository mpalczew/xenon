//! X11 taskbar icon.
//!
//! Panels such as tint2 paint an open window from `_NET_WM_ICON` only.
//! GPUI writes that property from [`gpui::WindowOptions::icon`] on X11 and
//! ignores the field on other platforms.

use std::sync::Arc;

const ICON_PNG: &[u8] = include_bytes!("../assets/xenon-icon-128.png");
const ICON_PX: u32 = 128;

/// Decode the bundled icon, or `None` if the bytes are not a PNG.
pub fn window_icon() -> Option<Arc<image::RgbaImage>> {
    match decode(ICON_PNG) {
        Ok(image) => Some(Arc::new(image)),
        Err(error) => {
            log::warn!("window icon unavailable: {error}");
            None
        }
    }
}

fn decode(png: &[u8]) -> image::ImageResult<image::RgbaImage> {
    let image = image::load_from_memory(png)?;
    let image = if image.width() == ICON_PX && image.height() == ICON_PX {
        image
    } else {
        image.resize(ICON_PX, ICON_PX, image::imageops::FilterType::Lanczos3)
    };
    Ok(image.into_rgba8())
}
