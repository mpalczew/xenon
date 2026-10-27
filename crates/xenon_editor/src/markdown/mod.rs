//! Markdown links: destinations under a cursor and in-document anchors. The
//! rendered view lives in `crate::prose`.

mod link;

pub use link::{LinkTarget, heading_offset, link_at};

use pulldown_cmark::Options;

pub(super) fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_DEFINITION_LIST
}
