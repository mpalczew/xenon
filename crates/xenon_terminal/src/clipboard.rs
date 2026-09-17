use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use gpui::{ClipboardEntry, ClipboardItem, Image, ImageFormat};
use image::ImageFormat as EncodedImageFormat;

pub(crate) fn clean_agent_output(text: &str) -> String {
    let text = strip_terminal_control_sequences(text);
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    let content: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|line| !line.trim().is_empty())
        .collect();
    let quote_block = !content.is_empty()
        && content.iter().all(|line| {
            let line = line.trim_start();
            line == ">" || line.starts_with("> ")
        });
    if quote_block {
        for line in &mut lines {
            let trimmed = line.trim_start();
            *line = trimmed
                .strip_prefix("> ")
                .or_else(|| trimmed.strip_prefix('>'))
                .unwrap_or(trimmed)
                .to_string();
        }
    }
    let cleaned: Vec<(String, bool)> = lines
        .iter()
        .map(|line| {
            let (text, wrapped) = strip_box_gutters(line);
            (strip_status_suffix(&text), wrapped)
        })
        .collect();
    let mut lines = unwrap_box_lines(cleaned);
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Return fenced code blocks, or cleaned text when the selection has no fence.
pub(crate) fn extract_code(text: &str) -> String {
    let stripped = strip_terminal_control_sequences(text);
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut in_block = false;
    for line in stripped.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed
            .chars()
            .next()
            .filter(|c| matches!(c, '│' | '┃' | '║' | '|'))
            .map(|border| trimmed[border.len_utf8()..].trim_start())
            .unwrap_or(trimmed);
        if marker.starts_with("```") || marker.starts_with("~~~") {
            if in_block {
                blocks.push(current.join("\n"));
            }
            current.clear();
            in_block = !in_block;
        } else if in_block {
            current.push(line);
        }
    }
    if in_block && !current.is_empty() {
        blocks.push(current.join("\n"));
    }
    if blocks.is_empty() {
        clean_agent_output(text)
    } else {
        blocks.join("\n\n")
    }
}

/// Strip one leading/trailing box-drawing gutter. `true` means the line filled
/// the TUI box (trailing border after padding) and the next line may wrap.
fn strip_box_gutters(line: &str) -> (String, bool) {
    let is_border = |ch: char| matches!(ch, '│' | '┃' | '║');
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return (String::new(), false);
    }
    if trimmed.chars().all(is_border) {
        return (String::new(), false);
    }

    let mut s = line;
    if let Some(ch) = s.trim_start().chars().next().filter(|c| is_border(*c)) {
        s = s.trim_start()[ch.len_utf8()..].trim_start();
    }
    let s = s.trim_end();
    let Some(ch) = s.chars().last().filter(|c| is_border(*c)) else {
        return (s.to_string(), false);
    };
    let without = &s[..s.len() - ch.len_utf8()];
    let wrapped = without.ends_with("  ");
    (without.trim().to_string(), wrapped)
}

fn unwrap_box_lines(lines: Vec<(String, bool)>) -> Vec<String> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for (text, wrapped) in lines {
        if text.is_empty() {
            if let Some(prev) = out.last_mut() {
                prev.1 = false;
            }
            out.push((text, false));
            continue;
        }
        if let Some(prev) = out.last_mut()
            && prev.1
            && !prev.0.is_empty()
        {
            prev.0.push(' ');
            prev.0.push_str(&text);
            prev.1 = wrapped;
            continue;
        }
        out.push((text, wrapped));
    }
    out.into_iter().map(|(text, _)| text).collect()
}

fn strip_terminal_control_sequences(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&']') {
                chars.next();
                while let Some(ch) = chars.next() {
                    if ch == '\u{7}' {
                        break;
                    }
                    if ch == '\u{1b}' && chars.next_if_eq(&'\\').is_some() {
                        break;
                    }
                }
            } else {
                let _ = chars.next_if_eq(&'[');
                for ch in chars.by_ref() {
                    if ('@'..='~').contains(&ch) {
                        break;
                    }
                }
            }
        } else if !ch.is_control() || matches!(ch, '\n' | '\r' | '\t') {
            result.push(ch);
        }
    }
    result.replace('\r', "")
}

fn strip_status_suffix(line: &str) -> String {
    let without_cursor = line
        .trim_end()
        .trim_end_matches(['█', '▌', '▋', '▊', '▉'])
        .trim_end();
    let Some((prefix, suffix)) = without_cursor.rsplit_once("  ") else {
        return without_cursor.to_string();
    };
    let time = suffix
        .strip_suffix(" AM")
        .or_else(|| suffix.strip_suffix(" PM"))
        .unwrap_or(suffix);
    let looks_like_time = time.split_once(':').is_some_and(|(hour, minute)| {
        !hour.is_empty()
            && !minute.is_empty()
            && hour.chars().all(|c| c.is_ascii_digit())
            && minute.chars().take(2).all(|c| c.is_ascii_digit())
    });
    if looks_like_time {
        prefix.trim_end().to_string()
    } else {
        without_cursor.to_string()
    }
}

pub(crate) fn terminal_clipboard_text(item: ClipboardItem) -> Option<String> {
    for entry in &item.entries {
        if let ClipboardEntry::ExternalPaths(paths) = entry {
            return Some(terminal_paths_text(paths.0.iter()));
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

/// Shell-quoted path list for paste or drag-drop into the PTY.
pub(crate) fn terminal_paths_text(paths: impl IntoIterator<Item = impl AsRef<Path>>) -> String {
    paths
        .into_iter()
        .map(shell_path)
        .collect::<Vec<_>>()
        .join(" ")
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

    #[test]
    fn clean_agent_output_strips_quotes_and_tui_chrome() {
        assert_eq!(
            clean_agent_output("> line one\n> line two\n> line three"),
            "line one\nline two\nline three"
        );
        assert_eq!(
            clean_agent_output(
                "│   line one                                      2:47 PM  █\n│   line two                                              █"
            ),
            "line one\nline two"
        );
        assert_eq!(
            clean_agent_output("\u{1b}[31mred\u{1b}[0m and more"),
            "red and more"
        );
        let dirty = "  Keep this for the “more” bias:                                      │\n\
│\n\
│ Be thorough. Reuse what still applies.                             │\n\
│\n\
Replace the exploration line with this:                             │\n\
│\n\
│ A stated need is not an implementation request. make the change             │\n\
│ when asked.";
        assert_eq!(
            clean_agent_output(dirty),
            "Keep this for the “more” bias:\n\n\
Be thorough. Reuse what still applies.\n\n\
Replace the exploration line with this:\n\n\
A stated need is not an implementation request. make the change when asked."
        );
        assert_eq!(
            clean_agent_output("| a | b |\n| 1 | 2 |"),
            "| a | b |\n| 1 | 2 |"
        );
    }

    #[test]
    fn extract_code_returns_fenced_blocks() {
        assert_eq!(
            extract_code("before\n```rust\nfn main() {}\n```\nafter"),
            "fn main() {}"
        );
        assert_eq!(
            extract_code("```\ntimeout  10:00 AM\n| a | b |\n```"),
            "timeout  10:00 AM\n| a | b |"
        );
    }
}
