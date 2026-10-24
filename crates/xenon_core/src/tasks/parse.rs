//! tasks.json (JSONC) to `TaskSet`: OS overrides, file-level globals, and the
//! legacy 0.1.0 fields, merged at the JSON value level the way VS Code does.

use std::collections::BTreeMap;

use serde_json_lenient::{Map, Value};

use super::model::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    Linux,
    Windows,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }

    fn key(self) -> &'static str {
        match self {
            Os::Mac => "osx",
            Os::Linux => "linux",
            Os::Windows => "windows",
        }
    }
}

const OS_KEYS: [&str; 3] = ["osx", "linux", "windows"];
/// File-level properties a task inherits when it does not set its own.
const INHERITED: [&str; 6] = [
    "type",
    "options",
    "presentation",
    "problemMatcher",
    "isBackground",
    "runOptions",
];

/// Parse a tasks.json document (comments and trailing commas allowed).
pub fn parse_task_set(json: &str, os: Os) -> Result<TaskSet, String> {
    let root: Value = serde_json_lenient::from_str(json).map_err(|e| e.to_string())?;
    let Value::Object(mut file) = root else {
        return Err("tasks.json must be a JSON object".into());
    };
    apply_os(&mut file, os);
    let legacy = file
        .get("version")
        .and_then(Value::as_str)
        .is_some_and(|v| v.starts_with("0."));
    let mut set = TaskSet::default();
    for input in file
        .get("inputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match parse_input(input) {
            Some(def) => {
                set.inputs.insert(def.id.clone(), def);
            }
            None => set.warnings.push("skipped an input without an id".into()),
        }
    }
    let raw_tasks = file.get("tasks").and_then(Value::as_array);
    for (i, raw) in raw_tasks.into_iter().flatten().enumerate() {
        let Value::Object(task) = raw else {
            set.warnings
                .push(format!("task {} is not an object", i + 1));
            continue;
        };
        let merged = merge_task(&file, task, os, legacy);
        match parse_task(&merged) {
            Ok(def) if set.get(&def.label).is_some() => {
                set.warnings
                    .push(format!("duplicate task label {:?}", def.label));
            }
            Ok(def) => set.tasks.push(def),
            Err(e) => set.warnings.push(format!("task {}: {e}", i + 1)),
        }
    }
    Ok(set)
}

fn apply_os(obj: &mut Map<String, Value>, os: Os) {
    let over = obj.get(os.key()).cloned();
    for key in OS_KEYS {
        obj.remove(key);
    }
    if let Some(Value::Object(over)) = over {
        for (k, v) in over {
            merge_key(obj, k, v);
        }
    }
}

fn merge_key(obj: &mut Map<String, Value>, key: String, over: Value) {
    match (obj.get_mut(&key), over) {
        (Some(Value::Object(base)), Value::Object(over)) => {
            for (k, v) in over {
                merge_key(base, k, v);
            }
        }
        (_, over) => {
            obj.insert(key, over);
        }
    }
}

fn merge_task(
    file: &Map<String, Value>,
    task: &Map<String, Value>,
    os: Os,
    legacy: bool,
) -> Map<String, Value> {
    let mut out = Map::new();
    for key in INHERITED {
        if let Some(v) = file.get(key) {
            out.insert(key.to_string(), v.clone());
        }
    }
    let mut own = task.clone();
    apply_os(&mut own, os);
    let file_command = file.get("command").cloned();
    let file_args = file.get("args").and_then(Value::as_array).cloned();
    for (k, v) in own.clone() {
        merge_key(&mut out, k, v);
    }
    if !own.contains_key("command")
        && let Some(command) = file_command
    {
        out.insert("command".into(), command);
        let mut args = file_args.unwrap_or_default();
        let name = own.get("taskName").or_else(|| own.get("label"));
        let suppress = own.get("suppressTaskName").and_then(Value::as_bool);
        let suppress = suppress.or_else(|| file.get("suppressTaskName").and_then(Value::as_bool));
        if legacy
            && suppress != Some(true)
            && let Some(name) = name
        {
            args.push(name.clone());
        }
        args.extend(
            own.get("args")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        );
        out.insert("args".into(), Value::Array(args));
    }
    out
}

fn parse_task(t: &Map<String, Value>) -> Result<TaskDef, String> {
    let label = str_of(t, "label")
        .or_else(|| str_of(t, "taskName"))
        .filter(|l| !l.is_empty())
        .ok_or("missing label")?;
    let mut unsupported = None;
    let type_name = str_of(t, "type").unwrap_or_else(|| "shell".into());
    let kind = match type_name.as_str() {
        "shell" => TaskKind::Shell,
        "process" => TaskKind::Process,
        other => {
            unsupported = Some(format!(
                "type {other:?} comes from a VS Code extension; Xenon runs shell and process tasks"
            ));
            TaskKind::Shell
        }
    };
    let command = t
        .get("command")
        .and_then(quoted)
        .filter(|c| !c.value.is_empty());
    let args = t
        .get("args")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(quoted).collect())
        .unwrap_or_default();
    let (depends_on, dep_problem) = depends_on(t.get("dependsOn"));
    if unsupported.is_none() {
        unsupported = dep_problem;
    }
    if unsupported.is_none() && command.is_none() && depends_on.is_empty() {
        unsupported = Some("no command".into());
    }
    let (is_background, background) = background_of(t);
    let run_on = t
        .get("runOptions")
        .and_then(|r| r.get("runOn"))
        .and_then(Value::as_str);
    Ok(TaskDef {
        label,
        detail: str_of(t, "detail"),
        kind,
        command,
        args,
        options: options_of(t.get("options")),
        presentation: presentation_of(t),
        group: group_of(t),
        depends_on,
        depends_sequence: str_of(t, "dependsOrder").as_deref() == Some("sequence"),
        is_background,
        background,
        run_on_folder_open: run_on == Some("folderOpen"),
        hide: t.get("hide").and_then(Value::as_bool).unwrap_or(false),
        unsupported,
    })
}

fn str_of(t: &Map<String, Value>, key: &str) -> Option<String> {
    t.get(key).and_then(Value::as_str).map(str::to_string)
}

fn quoted(v: &Value) -> Option<Quoted> {
    match v {
        Value::String(s) => Some(Quoted {
            value: s.clone(),
            quoting: None,
        }),
        Value::Object(o) => {
            let value = o.get("value").and_then(Value::as_str)?.to_string();
            let quoting = match o.get("quoting").and_then(Value::as_str) {
                Some("escape") => Some(Quoting::Escape),
                Some("strong") => Some(Quoting::Strong),
                Some("weak") => Some(Quoting::Weak),
                _ => None,
            };
            Some(Quoted { value, quoting })
        }
        _ => None,
    }
}

fn depends_on(v: Option<&Value>) -> (Vec<String>, Option<String>) {
    let items: Vec<&Value> = match v {
        None | Some(Value::Null) => return (Vec::new(), None),
        Some(Value::Array(a)) => a.iter().collect(),
        Some(other) => vec![other],
    };
    let mut labels = Vec::new();
    let mut problem = None;
    for item in items {
        match item {
            Value::String(s) => labels.push(s.clone()),
            _ => {
                problem = Some(
                    "dependsOn by task identifier ({type, ...}) is not supported; use labels"
                        .into(),
                )
            }
        }
    }
    (labels, problem)
}

fn options_of(v: Option<&Value>) -> TaskOptions {
    let Some(Value::Object(o)) = v else {
        return TaskOptions::default();
    };
    let env: BTreeMap<String, String> = o
        .get("env")
        .and_then(Value::as_object)
        .map(|e| {
            e.iter()
                .map(|(k, v)| {
                    let text = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    (k.clone(), text)
                })
                .collect()
        })
        .unwrap_or_default();
    let shell = o
        .get("shell")
        .and_then(Value::as_object)
        .map(|s| ShellOptions {
            executable: str_of(s, "executable"),
            args: s.get("args").and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            }),
        });
    TaskOptions {
        cwd: str_of(o, "cwd"),
        env,
        shell,
    }
}

fn presentation_of(t: &Map<String, Value>) -> Presentation {
    let mut p = Presentation::default();
    match t.get("showOutput").and_then(Value::as_str) {
        Some("silent") => p.reveal = Reveal::Silent,
        Some("never") => p.reveal = Reveal::Never,
        _ => {}
    }
    if let Some(echo) = t.get("echoCommand").and_then(Value::as_bool) {
        p.echo = echo;
    }
    let Some(Value::Object(o)) = t.get("presentation") else {
        return p;
    };
    match o.get("reveal").and_then(Value::as_str) {
        Some("always") => p.reveal = Reveal::Always,
        Some("silent") => p.reveal = Reveal::Silent,
        Some("never") => p.reveal = Reveal::Never,
        _ => {}
    }
    match o.get("panel").and_then(Value::as_str) {
        Some("shared") => p.panel = Panel::Shared,
        Some("dedicated") => p.panel = Panel::Dedicated,
        Some("new") => p.panel = Panel::New,
        _ => {}
    }
    let flag = |key: &str, slot: &mut bool| {
        if let Some(b) = o.get(key).and_then(Value::as_bool) {
            *slot = b;
        }
    };
    flag("focus", &mut p.focus);
    flag("clear", &mut p.clear);
    flag("close", &mut p.close);
    flag("echo", &mut p.echo);
    flag("showReuseMessage", &mut p.show_reuse_message);
    p
}

fn group_of(t: &Map<String, Value>) -> Option<Group> {
    let kind_of = |s: &str| match s {
        "build" => Some(GroupKind::Build),
        "test" => Some(GroupKind::Test),
        _ => None,
    };
    match t.get("group") {
        Some(Value::String(s)) => kind_of(s).map(|kind| Group {
            kind,
            is_default: false,
        }),
        Some(Value::Object(o)) => {
            let kind = kind_of(o.get("kind")?.as_str()?)?;
            let is_default = o.get("isDefault").and_then(Value::as_bool).unwrap_or(false);
            Some(Group { kind, is_default })
        }
        _ if t.get("isBuildCommand").and_then(Value::as_bool) == Some(true) => Some(Group {
            kind: GroupKind::Build,
            is_default: false,
        }),
        _ if t.get("isTestCommand").and_then(Value::as_bool) == Some(true) => Some(Group {
            kind: GroupKind::Test,
            is_default: false,
        }),
        _ => None,
    }
}

fn background_of(t: &Map<String, Value>) -> (bool, Option<Background>) {
    let flag = t
        .get("isBackground")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let matchers: Vec<&Value> = match t.get("problemMatcher") {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(Value::Null) | None => Vec::new(),
        Some(other) => vec![other],
    };
    let found = matchers.into_iter().find_map(|m| match m {
        Value::String(name) => super::background::builtin(name),
        Value::Object(o) => {
            let bg = o.get("background").or_else(|| o.get("watching"));
            match bg {
                Some(Value::Object(b)) => Some(Background {
                    active_on_start: b
                        .get("activeOnStart")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    begins: pattern(b.get("beginsPattern")),
                    ends: pattern(b.get("endsPattern")),
                }),
                _ => o
                    .get("base")
                    .and_then(Value::as_str)
                    .and_then(super::background::builtin),
            }
        }
        _ => None,
    });
    (flag, found)
}

fn pattern(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("regexp").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

fn parse_input(v: &Value) -> Option<InputDef> {
    let o = v.as_object()?;
    let id = str_of(o, "id")?;
    let description = str_of(o, "description").unwrap_or_default();
    let default = o.get("default").and_then(Value::as_str).map(str::to_string);
    let kind = match o.get("type").and_then(Value::as_str) {
        Some("promptString") => InputKind::PromptString {
            password: o.get("password").and_then(Value::as_bool).unwrap_or(false),
        },
        Some("pickString") => InputKind::PickString {
            options: o
                .get("options")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(pick_option).collect())
                .unwrap_or_default(),
        },
        _ => InputKind::Command,
    };
    Some(InputDef {
        id,
        description,
        default,
        kind,
    })
}

fn pick_option(v: &Value) -> Option<PickOption> {
    match v {
        Value::String(s) => Some(PickOption {
            label: s.clone(),
            value: s.clone(),
        }),
        Value::Object(o) => {
            let value = o.get("value").and_then(Value::as_str)?.to_string();
            let label = str_of(o, "label").unwrap_or_else(|| value.clone());
            Some(PickOption { label, value })
        }
        _ => None,
    }
}
