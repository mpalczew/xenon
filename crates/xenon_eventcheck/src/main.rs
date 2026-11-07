//! Commit check for the GTK event rule (docs/project-state.md): no list model, row or
//! widget changes inside a GTK signal handler, here a closure passed to a `connect_*`
//! method. Code inside a deferral (`glib::idle_add_local_once` and friends) is fine.
//! Only direct calls are seen; a helper that mutates internally is not.
//! Escape: `// sync: <reason>` on the line of, or the line above, the call or the
//! `connect_*` call.

use std::{fs, process::ExitCode};

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprMethodCall, Macro};

/// Calls that change a model, a row or a widget tree.
const MUTATORS: &[&str] = &[
    "set_expanded",
    "remove",
    "remove_all",
    "append",
    "insert",
    "splice",
    "items_changed",
    "set_model",
    "set_child",
    "unparent",
    "destroy",
    "close",
    "set_selected",
    "select_row",
    "unselect_all",
    "select_item",
    "set_root",
    "close_page",
];

/// Calls whose closure runs after the current event.
const DEFERS: &[&str] = &[
    "idle_add_local_once",
    "idle_add_local",
    "idle_add_once",
    "timeout_add_local",
    "timeout_add_local_once",
    "timeout_add_seconds_local",
    "timeout_add_seconds_local_once",
    "spawn_future_local",
    "spawn_local",
];

struct Check<'a> {
    file: String,
    lines: Vec<&'a str>,
    /// The `connect_*` whose handler is being walked, outside any deferral.
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

/// The closure inside `glib::clone!(#[weak] a, move |..| ..)`.
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

/// `cell.borrow_mut().insert(..)` is a Rust collection, and `id.remove()` a
/// `glib::SourceId`; neither touches GTK.
fn plain_collection(call: &ExprMethodCall) -> bool {
    let borrowed = matches!(&*call.receiver, Expr::MethodCall(r) if r.method == "borrow_mut");
    borrowed || (call.method == "remove" && call.args.is_empty())
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
            && MUTATORS.contains(&name.as_str())
            && !plain_collection(call)
            && !self.marked(call.method.span())
        {
            let line = call.method.span().start().line;
            println!(
                "{}:{line}: `{name}` inside a `{handler}` handler; defer it with \
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
