//! The worklist file model lives in `xenon_store::worklist`; only the inline
//! Markdown drawing helper (it needs GPUI text marks) stays here.

pub(crate) mod inline;
pub use xenon_store::worklist::*;
