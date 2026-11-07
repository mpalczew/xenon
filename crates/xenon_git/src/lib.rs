//! Git facts, with no UI toolkit.
//!
//! `status` names the git facts (dirt level, changed files), `measure` runs git
//! to fill them in (branch, changed files, line counts), and `bar` sizes the
//! change bar. Frontends only draw the results.

pub mod bar;
pub mod measure;
mod row_text;
pub mod status;
pub mod watch;
