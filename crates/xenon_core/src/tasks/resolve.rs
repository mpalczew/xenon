//! `TaskDef` + `VarContext` to a spawnable `ResolvedTask`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::model::*;
use super::vars::{VarContext, expand};

/// A task ready to spawn as its own process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTask {
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub presentation: Presentation,
    pub is_background: bool,
    pub background: Option<Background>,
    /// Human-readable command line with secret input values blanked. The only
    /// form that may be printed, logged, or shown.
    pub display: String,
}

/// Default shell args when `options.shell.args` is absent: login + command, so
/// a Finder-launched Xenon still sees the user's PATH.
const DEFAULT_SHELL_ARGS: [&str; 2] = ["-l", "-c"];

pub fn resolve_task(
    def: &TaskDef,
    ctx: &VarContext,
    default_shell: &str,
) -> Result<ResolvedTask, String> {
    if let Some(reason) = &def.unsupported {
        return Err(format!("{}: {reason}", def.label));
    }
    resolve_inner(def, ctx, default_shell).map_err(|e| format!("{}: {e}", def.label))
}

fn resolve_inner(
    def: &TaskDef,
    ctx: &VarContext,
    default_shell: &str,
) -> Result<ResolvedTask, String> {
    let ex = |s: &str| expand(s, ctx);
    let command = def.command.as_ref().ok_or("no command")?;
    let command_text = ex(&command.value)?;
    let args = def
        .args
        .iter()
        .map(|a| Ok((ex(&a.value)?, a.quoting)))
        .collect::<Result<Vec<_>, String>>()?;
    let cwd = match &def.options.cwd {
        Some(c) => PathBuf::from(ex(c)?),
        None => ctx.workspace.to_path_buf(),
    };
    let cwd = if cwd.is_relative() {
        ctx.workspace.join(cwd)
    } else {
        cwd
    };
    let env = def
        .options
        .env
        .iter()
        .map(|(k, v)| Ok((k.clone(), ex(v)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let line = command_line(&command_text, command.quoting, &args);
    let (program, spawn_args) = match def.kind {
        TaskKind::Process => (command_text, args.into_iter().map(|(a, _)| a).collect()),
        TaskKind::Shell => {
            let shell = def.options.shell.clone().unwrap_or_default();
            let program = match shell.executable {
                Some(e) => ex(&e)?,
                None => default_shell.to_string(),
            };
            let mut shell_args = match shell.args {
                Some(a) => a.iter().map(|a| ex(a)).collect::<Result<Vec<_>, _>>()?,
                None => DEFAULT_SHELL_ARGS.iter().map(|s| s.to_string()).collect(),
            };
            shell_args.push(line.clone());
            (program, shell_args)
        }
    };
    let mut display = line;
    for secret in ctx.secret_values() {
        display = display.replace(secret, "••••");
    }
    Ok(ResolvedTask {
        label: def.label.clone(),
        program,
        args: spawn_args,
        cwd,
        env,
        presentation: def.presentation,
        is_background: def.is_background,
        background: def.background.clone(),
        display,
    })
}

/// Shell command line: the command as written, arguments quoted.
fn command_line(
    command: &str,
    quoting: Option<Quoting>,
    args: &[(String, Option<Quoting>)],
) -> String {
    let mut line = match quoting {
        Some(q) => quote(command, q),
        None => command.to_string(),
    };
    for (arg, q) in args {
        line.push(' ');
        line.push_str(&quote(arg, q.unwrap_or(Quoting::Strong)));
    }
    line
}

fn quote(s: &str, quoting: Quoting) -> String {
    if s.is_empty() {
        return "''".into();
    }
    match quoting {
        Quoting::Strong => {
            if s.bytes().all(safe_byte) {
                s.to_string()
            } else {
                format!("'{}'", s.replace('\'', "'\\''"))
            }
        }
        Quoting::Weak => format!("\"{}\"", s.replace('"', "\\\"")),
        Quoting::Escape => s
            .chars()
            .flat_map(|c| {
                let esc = !(c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c)) && c.is_ascii();
                esc.then_some('\\').into_iter().chain(Some(c))
            })
            .collect(),
    }
}

fn safe_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'_' | b'.' | b'/' | b':' | b'=' | b'@' | b'%' | b'+' | b','
        )
}

/// Command line as the picker and trust prompt show it. Variables are expanded
/// where the context allows and left as written otherwise.
pub fn preview_line(def: &TaskDef, ctx: &VarContext) -> String {
    let keep = |s: &str| expand(s, ctx).unwrap_or_else(|_| s.to_string());
    let Some(command) = &def.command else {
        return String::new();
    };
    let args: Vec<_> = def
        .args
        .iter()
        .map(|a| (keep(&a.value), a.quoting))
        .collect();
    let mut line = command_line(&keep(&command.value), command.quoting, &args);
    for secret in ctx.secret_values() {
        line = line.replace(secret, "••••");
    }
    line
}
