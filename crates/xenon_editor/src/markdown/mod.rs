//! Render a markdown document to gpui elements for the preview pane. Block
//! structure comes from `pulldown-cmark`; inline emphasis/links/code use
//! `StyledText` highlights; fenced code blocks are syntax-highlighted via the
//! tree-sitter highlighters, keyed by the fence language.
//!
//! YAML frontmatter (`---` … `---`) is parsed as a metadata block (not a
//! setext heading) and shown as a compact key/value panel.
//!
//! Preview text is selectable (drag / double-click / ⌘A) via [`PreviewState`].
//!
//! Prose sits in a centered reading column; code blocks may run wider. When the
//! pane has room, an outline of headings sits to the right (`[` / `]` step).

mod builder;
mod nav;
mod outline;
mod select;
mod selectable;
mod state;
mod table;
mod yaml;

pub use state::{PreviewEvent, PreviewState};

use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Pixels, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};
use pulldown_cmark::{HeadingLevel, Options, Parser};
use theme::ActiveTheme;

use self::builder::{Builder, Built};
use self::outline::{OUTLINE_W, Outline};

/// Reading column width in multiples of the base font size (~72 characters).
const MEASURE_EMS: f32 = 46.;
/// Code blocks may widen to this before they scroll sideways.
const WIDE_EMS: f32 = 64.;
const PAD: Pixels = px(32.);

/// Parsed preview: UI tree + plain texts + source ranges for selection/copy.
pub struct PreviewRender {
    pub element: AnyElement,
    pub plain_blocks: Vec<SharedString>,
    /// Byte ranges in the original markdown, parallel to `plain_blocks`.
    pub source_ranges: Vec<std::ops::Range<usize>>,
    pub source: SharedString,
    /// Outline shown beside the column (block index per heading).
    pub outline_blocks: Vec<usize>,
    pub outline_shown: bool,
    /// The pane has not been laid out yet; render again next frame.
    pub needs_layout: bool,
}

/// Render markdown `source` into a scrollable column of selectable blocks.
pub fn render(
    source: &str,
    base: Pixels,
    mono_family: &str,
    host: Entity<PreviewState>,
    cx: &App,
) -> PreviewRender {
    let theme = cx.theme().clone();
    let colors = theme.colors().clone();
    let mut builder = Builder::new(
        theme,
        builder::BuilderColors {
            accent: colors.text_accent,
            muted: colors.text_muted,
            code_bg: colors.surface_background,
            rule: colors.border,
            selection: colors.element_selected,
        },
        base,
        mono_family,
        host.clone(),
    );
    for (event, range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
        builder.event(event, range);
    }
    let Built {
        blocks,
        plain,
        source_ranges,
        outline,
    } = builder.finish();
    let entries = outline::entries(&outline);
    let nav = host.read(cx).nav();
    let fits = nav.outline_fits(measure(base) + PAD * 2. + OUTLINE_W, OUTLINE_W);
    let outline_shown = !entries.is_empty() && fits == Some(true);
    let outline_blocks: Vec<usize> = entries.iter().map(|entry| entry.block).collect();
    let column = reading_column(blocks, nav.scroll());
    let side = outline_shown.then(|| {
        let active = nav.active(&outline_blocks);
        outline::panel(&Outline { entries, active }, host.clone(), cx)
    });
    let element = div()
        .size_full()
        .min_w_0()
        .min_h_0()
        .flex()
        .text_size(base)
        .text_color(colors.text)
        .bg(colors.editor_background)
        .child(column)
        .children(side)
        .into_any_element();
    PreviewRender {
        element,
        plain_blocks: plain,
        source_ranges,
        source: SharedString::from(source.to_string()),
        outline_blocks,
        outline_shown,
        needs_layout: fits.is_none(),
    }
}

pub(super) fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_DEFINITION_LIST
}

/// Scrolling column; each child is one block so the outline can find headings.
fn reading_column(blocks: Vec<AnyElement>, scroll: &ScrollHandle) -> AnyElement {
    div()
        .id("md-preview")
        .flex_1()
        .h_full()
        .min_w_0()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .flex()
        .flex_col()
        .px(PAD)
        .py(PAD)
        .children(blocks)
        .into_any_element()
}

fn measure(base: Pixels) -> Pixels {
    base * MEASURE_EMS
}

/// Center `element` at reading width (narrower panes shrink it).
pub(super) fn prose_slot(element: impl IntoElement, base: Pixels) -> AnyElement {
    centered(div().w(measure(base)).max_w_full().min_w_0().child(element))
}

/// Center a code block at its own width, between reading and wide width;
/// anything wider scrolls sideways inside the block.
pub(super) fn wide_slot(
    element: impl IntoElement,
    content: Pixels,
    ix: usize,
    base: Pixels,
) -> AnyElement {
    let width = content.clamp(measure(base), base * WIDE_EMS);
    centered(
        div()
            .id(("md-wide", ix))
            .w(width)
            .max_w_full()
            .min_w_0()
            .overflow_x_scroll()
            .child(element),
    )
}

fn centered(inner: impl IntoElement) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .justify_center()
        .child(inner)
        .into_any_element()
}

/// Code block box: its content width, but never narrower than the column.
pub(super) fn wide_block(content: Pixels, base: Pixels) -> gpui::Div {
    div().w(content.max(measure(base)))
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

    /// Nested list items must see increasing open-list depth (preview pad uses this).
    #[test]
    fn nested_list_item_depths() {
        let src = "- a\n  - b\n    - c\n- d\n";
        let mut depth = 0u32;
        let mut item_depths = Vec::new();
        for event in Parser::new_ext(src, markdown_options()) {
            match event {
                Event::Start(Tag::List(_)) => depth += 1,
                Event::End(TagEnd::List(_)) => depth = depth.saturating_sub(1),
                Event::Start(Tag::Item) => item_depths.push(depth),
                _ => {}
            }
        }
        assert_eq!(item_depths, vec![1, 2, 3, 1]);
    }

    /// Mirrors builder list flush rules (tight lists: flush lead-in before nested List).
    fn list_preview_blocks(src: &str) -> Vec<(u32, String)> {
        let mut depth = 0u32;
        let mut stack: Vec<Option<u64>> = Vec::new();
        let mut inline = String::new();
        let mut blocks = Vec::new();
        let flush = |depth: u32, inline: &mut String, blocks: &mut Vec<(u32, String)>| {
            if inline.trim().is_empty() {
                inline.clear();
                return;
            }
            blocks.push((depth, std::mem::take(inline)));
        };
        for event in Parser::new_ext(src, markdown_options()) {
            match event {
                Event::Start(Tag::List(start)) => {
                    flush(depth, &mut inline, &mut blocks);
                    stack.push(start);
                    depth = stack.len() as u32;
                }
                Event::End(TagEnd::List(_)) => {
                    stack.pop();
                    depth = stack.len() as u32;
                }
                Event::Start(Tag::Item) => {
                    let marker = match stack.last_mut() {
                        Some(Some(n)) => {
                            let s = format!("{n}. ");
                            *n += 1;
                            s
                        }
                        _ => "• ".into(),
                    };
                    inline.push_str(&marker);
                }
                Event::Text(t) => inline.push_str(&t),
                Event::TaskListMarker(checked) => {
                    inline.push_str(if checked { "[x] " } else { "[ ] " });
                }
                Event::End(TagEnd::Item) => flush(depth, &mut inline, &mut blocks),
                Event::End(TagEnd::Paragraph) => flush(depth, &mut inline, &mut blocks),
                _ => {}
            }
        }
        blocks
    }

    #[test]
    fn tight_nested_list_splits_into_separate_blocks() {
        let blocks = list_preview_blocks("- a\n  - b\n    - c\n- d\n");
        assert_eq!(
            blocks,
            vec![
                (1, "• a".into()),
                (2, "• b".into()),
                (3, "• c".into()),
                (1, "• d".into()),
            ],
            "must not collapse nested tight items into one block"
        );
    }

    #[test]
    fn ordered_nested_list_markers() {
        let blocks = list_preview_blocks("1. a\n   1. b\n   2. c\n2. d\n");
        assert_eq!(
            blocks,
            vec![
                (1, "1. a".into()),
                (2, "1. b".into()),
                (2, "2. c".into()),
                (1, "2. d".into()),
            ]
        );
    }
}
