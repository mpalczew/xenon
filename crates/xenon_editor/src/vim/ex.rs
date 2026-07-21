//! Ex command line (`:`) parse and dispatch for good-enough vim.

use std::path::PathBuf;

use super::substitute;
use super::{HandleResult, Mode, VimState, edited, handled};
use crate::buffer::Buffer;

impl VimState {
    /// Start `:` prompt; from visual pre-fills `'<,'>`.
    pub(in crate::vim) fn begin_ex(&mut self, buffer: &mut Buffer) {
        self.search_draft = None;
        self.ex_status = None;
        let visual_lines = if self.mode.is_visual() {
            buffer.selection_range().map(|r| {
                let a = buffer.rope().char_to_line(r.start);
                let end = r
                    .end
                    .saturating_sub(1)
                    .min(buffer.rope().len_chars().saturating_sub(1));
                let b = buffer.rope().char_to_line(end);
                (a.min(b), a.max(b))
            })
        } else {
            None
        };
        let line = if visual_lines.is_some() {
            "'<,'>".to_string()
        } else {
            String::new()
        };
        if self.mode.is_visual() {
            self.mode = Mode::Normal;
            buffer.clear_selection();
        }
        self.ex_draft = Some(ExDraft { line, visual_lines });
        self.clear_pending();
    }

    pub(super) fn handle_ex_key(&mut self, buffer: &mut Buffer, key: &str) -> HandleResult {
        match key {
            "escape" => {
                self.ex_draft = None;
                handled(false)
            }
            "enter" => self.commit_ex(buffer),
            "backspace" => {
                if let Some(draft) = &mut self.ex_draft {
                    draft.line.pop();
                }
                handled(false)
            }
            _ => HandleResult::default(),
        }
    }

    pub(super) fn append_ex_char(&mut self, text: &str) -> HandleResult {
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if clean.is_empty() {
            return handled(false);
        }
        if let Some(draft) = &mut self.ex_draft {
            draft.line.push_str(&clean);
        }
        handled(false)
    }

    fn commit_ex(&mut self, buffer: &mut Buffer) -> HandleResult {
        let Some(draft) = self.ex_draft.take() else {
            return handled(false);
        };
        buffer.clear_selection();
        let cursor_line = buffer.rope().char_to_line(buffer.cursor());
        let line_count = buffer.rope().len_lines().max(1);
        let last_search = self.search.as_ref().map(|s| s.pattern.as_str());
        let effect = run(
            &draft.line,
            buffer,
            ExContext {
                last_search,
                last_replacement: &mut self.last_sub_replacement,
                cursor_line,
                line_count,
                visual_lines: draft.visual_lines,
                is_dirty: buffer.is_dirty(),
            },
        );
        match effect {
            ExEffect::Message(msg) => {
                self.ex_status = if msg.is_empty() { None } else { Some(msg) };
                handled(false)
            }
            ExEffect::Error(msg) => {
                self.ex_status = Some(msg);
                handled(false)
            }
            ExEffect::Edited { message } => {
                self.ex_status = Some(message);
                edited()
            }
            other => HandleResult {
                handled: true,
                edited: false,
                ex: Some(other),
                ..Default::default()
            },
        }
    }
}

/// Active `:` prompt (mirrors search draft UX).
pub struct ExDraft {
    pub line: String,
    /// Inclusive 0-based line range when entered from visual (`'<,'>`).
    pub visual_lines: Option<(usize, usize)>,
}

/// Side effects the view/shell must handle after a successful command.
#[derive(Debug, Clone)]
pub enum ExEffect {
    /// Status / empty (no-op).
    Message(String),
    Error(String),
    /// Write buffer; optional new path (`:w path` / Save As).
    Write {
        force: bool,
        path: Option<PathBuf>,
    },
    /// `:wq` / `:x` — write then close.
    WriteQuit {
        force: bool,
    },
    /// Reload from disk (`:e` / `:e!`).
    Reload {
        force: bool,
    },
    /// Close tab (`:q` / `:q!`).
    Quit {
        force: bool,
    },
    /// Buffer was edited in place (`:s`).
    Edited {
        message: String,
    },
}

pub struct ExContext<'a> {
    pub last_search: Option<&'a str>,
    pub last_replacement: &'a mut String,
    /// Cursor line (0-based).
    pub cursor_line: usize,
    /// Line count (at least 1).
    pub line_count: usize,
    pub visual_lines: Option<(usize, usize)>,
    pub is_dirty: bool,
}

/// Run a finished ex line (without leading `:`).
pub fn run(line: &str, buffer: &mut Buffer, mut ctx: ExContext<'_>) -> ExEffect {
    let line = line.trim();
    if line.is_empty() {
        return ExEffect::Message(String::new());
    }

    let (range, rest) = parse_range_prefix(line, &ctx);
    let rest = rest.trim_start();
    if rest.is_empty() {
        let row = range.1.min(ctx.line_count.saturating_sub(1));
        buffer.set_cursor_position(row, 0);
        return ExEffect::Message(String::new());
    }

    // Quit-after-write before plain write (`wq` must not match `w`).
    if is_cmd(rest, "wq", 2) || is_cmd(rest, "xit", 1) || is_cmd(rest, "exit", 2) {
        return ExEffect::WriteQuit {
            force: has_bang(rest),
        };
    }
    if rest == "x" || rest == "x!" {
        return ExEffect::WriteQuit {
            force: rest.ends_with('!'),
        };
    }

    if is_cmd(rest, "quit", 1) {
        return ExEffect::Quit {
            force: has_bang(rest),
        };
    }

    if is_cmd(rest, "edit", 1) {
        return ExEffect::Reload {
            force: has_bang(rest),
        };
    }

    if is_cmd(rest, "write", 1) || is_cmd(rest, "update", 1) {
        if is_cmd(rest, "update", 1) && !ctx.is_dirty && write_path_arg(rest).is_none() {
            return ExEffect::Message(String::new());
        }
        return ExEffect::Write {
            force: has_bang(rest),
            path: write_path_arg(rest),
        };
    }

    if is_cmd(rest, "substitute", 1) {
        return run_substitute(cmd_arg_body(rest), buffer, range, &mut ctx);
    }

    ExEffect::Error(format!("not an editor command: {rest}"))
}

fn run_substitute(
    body: &str,
    buffer: &mut Buffer,
    range: (usize, usize),
    ctx: &mut ExContext<'_>,
) -> ExEffect {
    let body = body.trim_start();
    let cmd = match substitute::parse_substitute(body) {
        Ok(c) => c,
        Err(e) => return ExEffect::Error(e),
    };
    match substitute::apply_substitute(buffer, &cmd, range, ctx.last_search, ctx.last_replacement) {
        Ok(out) => {
            if out.replacements == 0 {
                ExEffect::Message("Pattern not found".into())
            } else {
                ExEffect::Edited {
                    message: format!(
                        "{} substitution{} on {} line{}",
                        out.replacements,
                        if out.replacements == 1 { "" } else { "s" },
                        out.lines_changed,
                        if out.lines_changed == 1 { "" } else { "s" }
                    ),
                }
            }
        }
        Err(e) => ExEffect::Error(e),
    }
}

/// Inclusive 0-based line range + remainder.
fn parse_range_prefix<'a>(line: &'a str, ctx: &ExContext<'_>) -> ((usize, usize), &'a str) {
    let last = ctx.line_count.saturating_sub(1);
    let cur = ctx.cursor_line.min(last);

    if let Some(rest) = line.strip_prefix('%') {
        return ((0, last), rest);
    }
    if let Some(rest) = line.strip_prefix("'<,'>") {
        if let Some(v) = ctx.visual_lines {
            return (v, rest);
        }
        return ((cur, cur), rest);
    }

    let parse_addr = |from: usize| -> Option<(usize, usize)> {
        let s = &line[from..];
        if s.starts_with('.') {
            return Some((cur, from + 1));
        }
        if s.starts_with('$') {
            return Some((last, from + 1));
        }
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        let n: usize = digits.parse().ok()?;
        let line0 = n.max(1) - 1;
        Some((line0.min(last), from + digits.len()))
    };

    if let Some((a, next)) = parse_addr(0) {
        if line[next..].starts_with(',')
            && let Some((b, next2)) = parse_addr(next + 1)
        {
            return ((a.min(b), a.max(b)), &line[next2..]);
        }
        return ((a, a), &line[next..]);
    }

    ((cur, cur), line)
}

fn cmd_token(rest: &str) -> &str {
    let end = rest
        .char_indices()
        .find(|(_, c)| c.is_whitespace() || *c == '/')
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    &rest[..end]
}

fn has_bang(rest: &str) -> bool {
    cmd_token(rest).ends_with('!')
}

fn is_cmd(rest: &str, name: &str, min_prefix: usize) -> bool {
    let token = cmd_token(rest);
    let token = token.strip_suffix('!').unwrap_or(token);
    if token.len() < min_prefix {
        return false;
    }
    name.starts_with(token)
}

/// Argument after `w` / `w!` / `write` token (path or empty).
fn write_path_arg(rest: &str) -> Option<PathBuf> {
    let token = cmd_token(rest);
    let after = rest[token.len()..].trim_start();
    if after.is_empty() {
        None
    } else {
        Some(PathBuf::from(after))
    }
}

/// Body after command name (`s/foo/bar` → `/foo/bar`).
fn cmd_arg_body(rest: &str) -> &str {
    let token = cmd_token(rest);
    &rest[token.len()..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn buffer_with(text: &str) -> Buffer {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        Buffer::open(file.path()).unwrap()
    }

    fn ctx<'a>(prev: &'a mut String, lines: usize) -> ExContext<'a> {
        ExContext {
            last_search: None,
            last_replacement: prev,
            cursor_line: 0,
            line_count: lines,
            visual_lines: None,
            is_dirty: false,
        }
    }

    #[test]
    fn percent_substitute() {
        let mut buf = buffer_with("a a\nb a\n");
        let mut prev = String::new();
        let effect = run("%s/a/X/g", &mut buf, ctx(&mut prev, 2));
        assert!(matches!(effect, ExEffect::Edited { .. }), "{effect:?}");
        assert_eq!(buf.text(), "X X\nb X\n");
    }

    #[test]
    fn write_quit_parse() {
        let mut buf = buffer_with("x\n");
        let mut prev = String::new();
        assert!(matches!(
            run("w", &mut buf, ctx(&mut prev, 1)),
            ExEffect::Write {
                force: false,
                path: None
            }
        ));
        assert!(matches!(
            run("w!", &mut buf, ctx(&mut prev, 1)),
            ExEffect::Write {
                force: true,
                path: None
            }
        ));
        assert!(matches!(
            run("wq", &mut buf, ctx(&mut prev, 1)),
            ExEffect::WriteQuit { force: false }
        ));
        assert!(matches!(
            run("q!", &mut buf, ctx(&mut prev, 1)),
            ExEffect::Quit { force: true }
        ));
    }

    #[test]
    fn write_path() {
        let mut buf = buffer_with("x\n");
        let mut prev = String::new();
        match run("w /tmp/out.txt", &mut buf, ctx(&mut prev, 1)) {
            ExEffect::Write {
                force: false,
                path: Some(p),
            } => assert_eq!(p, PathBuf::from("/tmp/out.txt")),
            other => panic!("{other:?}"),
        }
    }
}
