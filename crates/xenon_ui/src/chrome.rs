//! App-local compatibility surface for the shared Xenon design system.
//! Feature composition stays in `xenon_ui`; visual primitives live in the
//! dependency so other Xenon surfaces can reuse the same vocabulary.

pub(crate) use xenon_design_system::{
    SelectionPaint, accent_surface, attention_color, list_selection, status_pip, tab_selection,
    working_color,
};
