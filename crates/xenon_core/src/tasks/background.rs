//! The `$tsc-watch` patterns are copied from VS Code (see ATTRIBUTION.md).
//!
//! Background-task readiness from `problemMatcher.background` patterns.
//!
//! Choice (see docs/designs/vscode-tasks.md): the patterns decide only when a
//! background dependency is ready, never problems. Built-in matcher names other
//! than `$tsc-watch` carry no patterns here, so such a task is ready on its
//! first output line.

use regex::Regex;

use super::model::Background;

const TSC_BEGINS: &str = r"^\s*(?:message TS6032:|\[?\d{1,2}:\d{1,2}:\d{1,2}(?: AM| PM)?\]? (?:- )?File change detected\. Starting incremental compilation\.\.\.)";
const TSC_ENDS: &str = r"^\s*(?:message TS6042:|\[?\d{1,2}:\d{1,2}:\d{1,2}(?: AM| PM)?\]? (?:- )?Found \d+ errors?\. Watching for file changes\.)";

/// Patterns for the built-in matchers Xenon knows.
pub fn builtin(name: &str) -> Option<Background> {
    match name {
        "$tsc-watch" => Some(Background {
            active_on_start: true,
            begins: Some(TSC_BEGINS.into()),
            ends: Some(TSC_ENDS.into()),
        }),
        _ => None,
    }
}

/// Feeds output lines and reports when the task first becomes ready.
pub struct Readiness {
    begins: Option<Regex>,
    ends: Option<Regex>,
    active: bool,
    ready: bool,
}

impl Readiness {
    /// `None` patterns (or an invalid regex) mean ready on first output.
    pub fn new(background: Option<&Background>) -> Self {
        let compile = |p: &Option<String>| p.as_deref().and_then(|p| Regex::new(p).ok());
        let (begins, ends, active_on_start) = match background {
            Some(b) => (compile(&b.begins), compile(&b.ends), b.active_on_start),
            None => (None, None, false),
        };
        let active = active_on_start || begins.is_none();
        Self {
            begins,
            ends,
            active,
            ready: false,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Feed complete output lines. True the first time the task becomes ready.
    pub fn feed(&mut self, lines: &[&str]) -> bool {
        if self.ready {
            return false;
        }
        for line in lines {
            if self.ends.is_none() {
                if !line.trim().is_empty() {
                    self.ready = true;
                }
            } else {
                if self.begins.as_ref().is_some_and(|b| b.is_match(line)) {
                    self.active = true;
                }
                if self.active && self.ends.as_ref().is_some_and(|e| e.is_match(line)) {
                    self.ready = true;
                }
            }
            if self.ready {
                return true;
            }
        }
        false
    }
}
