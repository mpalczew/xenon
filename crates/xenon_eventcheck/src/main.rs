//! Commit check for the GTK event rule (every crash so far broke it): a GTK signal handler,
//! here a closure passed to a `connect_*` method, may only read and defer. Outside a
//! deferral (`glib::idle_add_local_once` and friends) every call in it must be a read:
//! a getter or query (`get_*`, `is_*`, `has_*`, `upgrade`, `borrow`, `clone`, ...), an
//! Option/Result/iterator combinator, a constructor like `Some(..)`, or a call that only
//! updates our own Rust state (`Cell`, or a `borrow_mut()` collection). Anything else,
//! including our own helpers, changes widgets, models, buffers or styles while GTK may
//! be mid-layout, so it must move into a deferral.
//! Escape: `// sync: <reason>` on the line of, or the line above, the call or the
//! `connect_*` call (the latter covers the whole handler). Use it for exactly two reasons:
//! GTK needs an answer or an ordering before the handler returns (a return value, a
//! consumed key, an accepted drop, a list factory), or the read is tied to the moment of
//! the event (text-iter positions, the row under the pointer, a list index that may shift).
//! Otherwise move the work, reads included, into a deferral.

use std::{fs, process::ExitCode};

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprMethodCall, Macro};

const READS: &[&str] = &[
    "upgrade",
    "downgrade",
    "clone",
    "borrow",
    "get",
    "unwrap",
    "expect",
    "unwrap_or",
    "unwrap_or_default",
    "unwrap_or_else",
    "ok",
    "ok_or",
    "ok_or_else",
    "err",
    "map",
    "map_or",
    "map_or_else",
    "and_then",
    "or",
    "or_else",
    "filter",
    "cloned",
    "copied",
    "iter",
    "len",
    "contains",
    "starts_with",
    "ends_with",
    "eq",
    "ne",
    "cmp",
    "min",
    "max",
    "into",
    "from",
    "format",
    "trim",
    "split",
    "join",
    "chars",
    "bytes",
    "lines",
    "first",
    "last",
    "next",
    "count",
    "any",
    "all",
    "find",
    "position",
    "collect",
    "rev",
    "take",
    "skip",
    "zip",
    "enumerate",
    "sum",
    "abs",
    "powi",
    "round",
    "floor",
    "ceil",
    "clamp",
    "text",
    "label",
    "name",
    "path",
    "selected",
    "selected_item",
    "item",
    "keyval",
    "keycode",
    "state",
    "modifiers",
    "parent",
    "root",
    "ancestor",
    "child",
    "first_child",
    "last_child",
    "next_sibling",
    "prev_sibling",
    "display",
    "surface",
    "downcast",
    "downcast_ref",
    "downcast_ref_unchecked",
    "dynamic_cast_ref",
    "type_",
    "start_iter",
    "end_iter",
    "slice",
    "line",
    "line_offset",
    "offset",
    "bounds",
    "selection_bounds",
    "value",
    "lower",
    "upper",
    "page_size",
    "step_increment",
    "page_increment",
    "width",
    "height",
    "x",
    "y",
    "allocation",
    "compute_point",
    "compute_bounds",
    "translate_coordinates",
    "pick",
    "current_event",
    "widget",
    "stop_signal_emission_by_name",
    "stop_signal_emission",
    "set",
    "replace",
    "exists",
    "file_name",
    "extension",
    "borrow_mut",
    "intersects",
    "contains_point",
    "index",
    "abs_diff",
    "current_event_state",
    "and_downcast",
    "and_downcast_ref",
    "clipboard",
    "char_count",
    "basename",
    "with",
    "send",
    "active_name",
    "selected_page",
    "running",
    "waiting",
    "termprop_string",
    "remote_status",
    "str",
    "active",
    "active_tab",
];

const READ_PREFIXES: &[&str] = &[
    "get_", "is_", "has_", "can_", "as_", "to_", "try_", "iter_", "n_",
];

fn plain_call(call: &ExprCall) -> bool {
    let Expr::Path(p) = &*call.func else {
        return false;
    };
    let segs: Vec<String> = p
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    let last = segs.last().map(String::as_str).unwrap_or("");
    let upper = last.chars().next().is_some_and(char::is_uppercase);
    let constructor = segs.len() >= 2 && ["new", "default", "empty", "none"].contains(&last)
        || ["from_", "with_", "for_"]
            .iter()
            .any(|p| last.starts_with(p));
    upper
        || constructor
        || (segs.len() == 1 && ["drop", "Some", "Ok", "Err", "Box", "Rc"].contains(&last))
        || READS.contains(&last)
        || READ_PREFIXES.iter().any(|p| last.starts_with(p))
        || (segs.len() >= 2
            && [
                "Rc", "Arc", "Cell", "RefCell", "Box", "Some", "String", "Instant", "Duration",
                "Path", "PathBuf",
            ]
            .contains(&segs[segs.len() - 2].as_str()))
}

const DEFERS: &[&str] = &[
    "defer",
    "idle_add_local_once",
    "idle_add_local",
    "idle_add_once",
    "timeout_add_local",
    "timeout_add_local_once",
    "timeout_add_seconds_local",
    "timeout_add_seconds_local_once",
    "spawn_future_local",
    "spawn_local",
    "tab_event",
];

struct Check<'a> {
    file: String,
    lines: Vec<&'a str>,
    handler: Option<String>,
    found: usize,
}

impl Check<'_> {
    fn marked(&self, span: Span) -> bool {
        let line = span.start().line;
        (line.saturating_sub(2)..line)
            .any(|i| self.lines.get(i).is_some_and(|l| l.contains("// sync:")))
    }

    fn walk(&mut self, handler: Option<String>, f: impl FnOnce(&mut Self)) {
        let outer = std::mem::replace(&mut self.handler, handler);
        f(self);
        self.handler = outer;
    }
}

fn clone_closure(mac: &Macro) -> Option<Expr> {
    if !mac.path.segments.last().is_some_and(|s| s.ident == "clone") {
        return None;
    }
    let tokens: Vec<TokenTree> = mac.tokens.clone().into_iter().collect();
    let start = (0..tokens.len()).find(|&i| {
        let after_comma =
            i == 0 || matches!(&tokens[i - 1], TokenTree::Punct(p) if p.as_char() == ',');
        let opens = match &tokens[i] {
            TokenTree::Ident(id) => id == "move" || id == "async",
            TokenTree::Punct(p) => p.as_char() == '|',
            _ => false,
        };
        after_comma && opens
    })?;
    syn::parse2(tokens[start..].iter().cloned().collect::<TokenStream>()).ok()
}

fn read_only(call: &ExprMethodCall) -> bool {
    let name = call.method.to_string();
    let borrowed = matches!(&*call.receiver, Expr::MethodCall(r) if r.method == "borrow_mut");
    borrowed
        || (name == "remove" && call.args.is_empty())
        || READS.contains(&name.as_str())
        || READ_PREFIXES.iter().any(|p| name.starts_with(p))
}

fn last_segment(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

impl<'ast> Visit<'ast> for Check<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        let name = call.method.to_string();
        if let Some(handler) = &self.handler
            && !DEFERS.contains(&name.as_str())
            && !name.starts_with("connect_")
            && !read_only(call)
            && !self.marked(call.method.span())
        {
            let line = call.method.span().start().line;
            println!(
                "{}:{line}: `{name}` is not a read inside a `{handler}` handler; defer it with \
                 glib::idle_add_local_once or mark it `// sync: <reason>`",
                self.file
            );
            self.found += 1;
        }
        if DEFERS.contains(&name.as_str()) {
            self.visit_expr(&call.receiver);
            self.walk(None, |c| call.args.iter().for_each(|a| c.visit_expr(a)));
        } else if name.starts_with("connect_") {
            self.visit_expr(&call.receiver);
            let handler = (!self.marked(call.method.span())).then_some(name);
            for arg in &call.args {
                match arg {
                    Expr::Closure(_) => self.walk(handler.clone(), |c| c.visit_expr(arg)),
                    Expr::Macro(m) => match clone_closure(&m.mac) {
                        Some(closure) => self.walk(handler.clone(), |c| c.visit_expr(&closure)),
                        None => self.visit_expr(arg),
                    },
                    _ => self.visit_expr(arg),
                }
            }
        } else {
            visit::visit_expr_method_call(self, call);
        }
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if let Some(handler) = &self.handler
            && !plain_call(call)
            && !last_segment(&call.func).is_some_and(|f| DEFERS.contains(&f.as_str()))
            && !self.marked(call.func.span())
        {
            let line = call.func.span().start().line;
            println!(
                "{}:{line}: call is not a read inside a `{handler}` handler; defer it with \
                 glib::idle_add_local_once or mark it `// sync: <reason>`",
                self.file
            );
            self.found += 1;
        }
        if last_segment(&call.func).is_some_and(|f| DEFERS.contains(&f.as_str())) {
            self.walk(None, |c| call.args.iter().for_each(|a| c.visit_expr(a)));
        } else {
            visit::visit_expr_call(self, call);
        }
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        if let Some(closure) = clone_closure(mac) {
            self.visit_expr(&closure);
        }
    }
}

fn main() -> ExitCode {
    let mut found = 0;
    for entry in ignore::Walk::new(std::env::args().nth(1).unwrap_or("crates".into())).flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(ast) = syn::parse_file(&source) else {
            println!("{}: does not parse", path.display());
            found += 1;
            continue;
        };
        let mut check = Check {
            file: path.display().to_string(),
            lines: source.lines().collect(),
            handler: None,
            found: 0,
        };
        check.visit_file(&ast);
        found += check.found;
    }
    if found > 0 {
        println!("{found} change(s) inside GTK signal handlers (GTK event rule)");
        return ExitCode::FAILURE;
    }
    println!("gtk events ok");
    ExitCode::SUCCESS
}
