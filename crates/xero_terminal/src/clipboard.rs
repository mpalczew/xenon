use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use gpui::{ClipboardEntry, ClipboardItem, Image, ImageFormat};
use image::ImageFormat as EncodedImageFormat;

pub(crate) fn terminal_clipboard_text(item: ClipboardItem) -> Option<String> {
    for entry in &item.entries {
        if let ClipboardEntry::ExternalPaths(paths) = entry {
            return Some(paths.0.iter().map(shell_path).collect::<Vec<_>>().join(" "));
        }
    }

    for entry in &item.entries {
        if let ClipboardEntry::Image(image) = entry {
            return save_clipboard_image(image)
                .map(shell_path)
                .map_err(log_paste_error)
                .ok();
        }
    }

    item.text()
}

fn save_clipboard_image(image: &Image) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("xero-clipboard");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "clipboard-{}-{}.png",
        timestamp_nanos(),
        image.id()
    ));
    let bytes = image_as_png(image)?;
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn image_as_png(image: &Image) -> Result<Vec<u8>> {
    if image.format == ImageFormat::Png {
        return Ok(image.bytes.clone());
    }

    let format = encoded_format(image.format)?;
    let decoded = image::load_from_memory_with_format(&image.bytes, format)?;
    let mut png = Cursor::new(Vec::new());
    decoded.write_to(&mut png, EncodedImageFormat::Png)?;
    Ok(png.into_inner())
}

fn encoded_format(format: ImageFormat) -> Result<EncodedImageFormat> {
    match format {
        ImageFormat::Png => Ok(EncodedImageFormat::Png),
        ImageFormat::Jpeg => Ok(EncodedImageFormat::Jpeg),
        ImageFormat::Webp => Ok(EncodedImageFormat::WebP),
        ImageFormat::Gif => Ok(EncodedImageFormat::Gif),
        ImageFormat::Bmp => Ok(EncodedImageFormat::Bmp),
        ImageFormat::Tiff => Ok(EncodedImageFormat::Tiff),
        ImageFormat::Ico => Ok(EncodedImageFormat::Ico),
        ImageFormat::Pnm => Ok(EncodedImageFormat::Pnm),
        ImageFormat::Svg => anyhow::bail!("SVG clipboard images cannot be converted to PNG"),
    }
}

fn shell_path(path: impl AsRef<Path>) -> String {
    shell_quote(&path.as_ref().to_string_lossy())
}

fn shell_quote(text: &str) -> String {
    if text.bytes().all(is_shell_safe) {
        return text.to_string();
    }
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn is_shell_safe(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b':' | b'+' | b'=')
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn log_paste_error(error: anyhow::Error) {
    log::warn!("failed to paste clipboard image: {error:#}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_leaves_simple_paths_plain() {
        assert_eq!(shell_quote("/tmp/xero-image.png"), "/tmp/xero-image.png");
    }

    #[test]
    fn shell_quote_handles_spaces_and_quotes() {
        assert_eq!(shell_quote("/tmp/a b's.png"), "'/tmp/a b'\\''s.png'");
    }

    #[test]
    fn clipboard_paths_prefer_absolute_external_paths() {
        let item = ClipboardItem {
            entries: vec![
                ClipboardEntry::ExternalPaths(gpui::ExternalPaths(
                    [PathBuf::from("/tmp/a b.png"), PathBuf::from("/tmp/c.png")].into(),
                )),
                ClipboardEntry::String(gpui::ClipboardString::new("a b.png".into())),
            ],
        };

        assert_eq!(
            terminal_clipboard_text(item),
            Some("'/tmp/a b.png' /tmp/c.png".into())
        );
    }
}
