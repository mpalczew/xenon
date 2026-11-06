//! Git facts and the read-only diff model, with no UI toolkit.
//!
//! `status` names the git facts (dirt level, changed files), `measure` runs git
//! to fill them in (branch, changed files, line counts), `bar` sizes the change bar,
//! `diff` parses `git diff` output into hunks and rows, and `load` runs git to
//! build the diff for one changed file. Frontends only draw the results.

pub mod bar;
pub mod diff;
pub mod load;
pub mod measure;
pub mod status;
