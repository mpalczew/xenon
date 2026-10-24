//! Typed VS Code task definitions after OS overrides and file-level globals
//! have been merged in. Strings still contain `${...}` variables; see `vars`
//! and `resolve`.

use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Quoting {
    Escape,
    Strong,
    Weak,
}

/// A command or argument: plain text or `{ value, quoting }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Quoted {
    pub value: String,
    /// `None`: the VS Code default (strong for shell arguments).
    pub quoting: Option<Quoting>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TaskKind {
    Shell,
    Process,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ShellOptions {
    pub executable: Option<String>,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TaskOptions {
    pub cwd: Option<String>,
    pub env: BTreeMap<String, String>,
    pub shell: Option<ShellOptions>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Reveal {
    Always,
    Silent,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Panel {
    Shared,
    Dedicated,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Presentation {
    pub reveal: Reveal,
    pub focus: bool,
    pub panel: Panel,
    pub clear: bool,
    pub close: bool,
    pub echo: bool,
    pub show_reuse_message: bool,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            reveal: Reveal::Always,
            focus: false,
            panel: Panel::Shared,
            clear: false,
            close: false,
            echo: true,
            show_reuse_message: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum GroupKind {
    Build,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Group {
    pub kind: GroupKind,
    pub is_default: bool,
}

/// `problemMatcher.background` patterns, as regex source text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Background {
    pub active_on_start: bool,
    pub begins: Option<String>,
    pub ends: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskDef {
    pub label: String,
    pub detail: Option<String>,
    pub kind: TaskKind,
    pub command: Option<Quoted>,
    pub args: Vec<Quoted>,
    pub options: TaskOptions,
    pub presentation: Presentation,
    pub group: Option<Group>,
    pub depends_on: Vec<String>,
    pub depends_sequence: bool,
    pub is_background: bool,
    pub background: Option<Background>,
    pub run_on_folder_open: bool,
    pub hide: bool,
    /// Why this task cannot run in Xenon (extension-contributed type, no
    /// command, unsupported `dependsOn` form). Listed but not runnable.
    pub unsupported: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PickOption {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum InputKind {
    PromptString {
        password: bool,
    },
    PickString {
        options: Vec<PickOption>,
    },
    /// `command` inputs need a VS Code extension command: unsupported.
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputDef {
    pub id: String,
    pub description: String,
    pub default: Option<String>,
    pub kind: InputKind,
}

impl InputDef {
    pub fn is_secret(&self) -> bool {
        matches!(self.kind, InputKind::PromptString { password: true })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskSet {
    pub tasks: Vec<TaskDef>,
    pub inputs: BTreeMap<String, InputDef>,
    /// Tasks or sections skipped while parsing, one line each.
    pub warnings: Vec<String>,
}

impl TaskSet {
    pub fn get(&self, label: &str) -> Option<&TaskDef> {
        self.tasks.iter().find(|t| t.label == label)
    }

    /// The default task of a group, if the file marks one.
    pub fn default_of(&self, kind: GroupKind) -> Option<&TaskDef> {
        self.tasks
            .iter()
            .find(|t| t.group.is_some_and(|g| g.kind == kind && g.is_default))
    }
}

impl TaskDef {
    /// Every string that is expanded when the task runs.
    pub fn strings(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        out.extend(self.command.iter().map(|c| c.value.as_str()));
        out.extend(self.args.iter().map(|a| a.value.as_str()));
        out.extend(self.options.cwd.iter().map(String::as_str));
        out.extend(self.options.env.values().map(String::as_str));
        if let Some(shell) = &self.options.shell {
            out.extend(shell.executable.iter().map(String::as_str));
            out.extend(shell.args.iter().flatten().map(String::as_str));
        }
        out
    }
}
