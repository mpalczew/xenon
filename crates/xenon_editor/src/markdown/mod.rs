//! Render a markdown document to gpui elements for the preview pane. Block
//! structure comes from `pulldown-cmark`; inline emphasis/links/code use
//! `StyledText` highlights; fenced code blocks are syntax-highlighted via the
//! tree-sitter highlighters, keyed by the fence language.
//!
//! YAML frontmatter (`---` … `---`) is parsed as a metadata block (not a
//! setext heading) and shown as a compact key/value panel.

mod builder;
mod table;
mod yaml;

use gpui::{
    AnyElement, App, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, div, px,
};
use pulldown_cmark::{HeadingLevel, Options, Parser};
use theme::ActiveTheme;

use self::builder::Builder;

/// Render markdown `source` into a scrollable column of block elements sized
/// relative to `base`, using `mono_family` for code spans and fences.
pub fn render(source: &str, base: Pixels, mono_family: &str, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let colors = theme.colors().clone();
    let mut builder = Builder::new(
        theme,
        builder::BuilderColors {
            accent: colors.text_accent,
            muted: colors.text_muted,
            code_bg: colors.surface_background,
            rule: colors.border,
        },
        base,
        mono_family,
    );
    for event in Parser::new_ext(source, markdown_options()) {
        builder.event(event);
    }
    preview_surface(
        builder.into_blocks(),
        base,
        colors.text,
        colors.editor_background,
    )
}

pub(super) fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_DEFINITION_LIST
}

fn preview_surface(
    blocks: Vec<AnyElement>,
    base: Pixels,
    text: Hsla,
    background: Hsla,
) -> AnyElement {
    div()
        .id("md-preview")
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_x_scroll()
        .overflow_y_scroll()
        .p_4()
        .text_size(base)
        .text_color(text)
        .bg(background)
        .children(blocks)
        .into_any_element()
}

pub(super) fn wide_block(width: Pixels) -> gpui::Div {
    div().w(width)
}

pub(super) fn code_block_width(code: &str, base: Pixels) -> Pixels {
    let cols = code.lines().map(str::len).max().unwrap_or(1).max(32);
    px(cols as f32 * f32::from(base) * 0.62 + 32.)
}

pub(super) fn heading_size(base: Pixels, level: HeadingLevel) -> Pixels {
    let scale = match level {
        HeadingLevel::H1 => 1.7,
        HeadingLevel::H2 => 1.45,
        HeadingLevel::H3 => 1.25,
        HeadingLevel::H4 => 1.15,
        _ => 1.05,
    };
    px(f32::from(base) * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Event, Tag, TagEnd};

    #[test]
    fn parser_metadata_not_setext_heading() {
        let src = "---\nname: brainstorming\ndescription: Open the problem space.\n---\n\n# Body\n";
        let mut saw_meta = false;
        let mut saw_setext_h2 = false;
        for event in Parser::new_ext(src, markdown_options()) {
            match event {
                Event::Start(Tag::MetadataBlock(_)) => saw_meta = true,
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                }) => saw_setext_h2 = true,
                Event::End(TagEnd::MetadataBlock(_)) => {}
                _ => {}
            }
        }
        assert!(saw_meta, "expected MetadataBlock with yaml option");
        assert!(!saw_setext_h2, "description must not become setext H2");
    }

    #[test]
    fn parser_without_yaml_flag_makes_setext() {
        let src = "---\nname: brainstorming\ndescription: Open the problem space.\n---\n";
        let mut saw_h2 = false;
        for event in Parser::new_ext(src, Options::empty()) {
            if matches!(
                event,
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                })
            ) {
                saw_h2 = true;
            }
        }
        assert!(saw_h2, "baseline: no yaml flag yields setext H2");
    }
}
