use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json_lenient::{Map, Value};

use super::model::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Os {
    Linux,
}

impl Os {
    pub fn current() -> Self {
        Os::Linux
    }

    fn key(self) -> &'static str {
        match self {
            Os::Linux => "linux",
        }
    }
}

/// Override keys VS Code files may carry; only `linux` applies here, the rest
/// are dropped.
const OS_KEYS: [&str; 3] = ["osx", "linux", "windows"];
const INHERITED: [&str; 5] = [
    "type",
    "options",
    "presentation",
    "isBackground",
    "runOptions",
];

pub(crate) fn parse_task_set(json: &str, os: Os) -> Result<TaskSet, String> {
    let root: Value = serde_json_lenient::from_str(json).map_err(|e| e.to_string())?;
    let Value::Object(mut file) = root else {
        return Err("tasks.json must be a JSON object".into());
    };
    apply_os(&mut file, os);
    let mut set = TaskSet::default();
    for input in file
        .get("inputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match serde_json_lenient::from_value::<RawInput>(input.clone()) {
            Ok(raw) => {
                let def = raw.into_def();
                set.inputs.insert(def.id.clone(), def);
            }
            Err(_) => set.warnings.push("skipped an input without an id".into()),
        }
    }
    let raw_tasks = file.get("tasks").and_then(Value::as_array);
    for (i, raw) in raw_tasks.into_iter().flatten().enumerate() {
        let Value::Object(task) = raw else {
            set.warnings
                .push(format!("task {} is not an object", i + 1));
            continue;
        };
        let merged = Value::Object(merge_task(&file, task, os));
        match serde_json_lenient::from_value::<RawTask>(merged)
            .map_err(|e| e.to_string())
            .and_then(RawTask::into_def)
        {
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

fn merge_task(file: &Map<String, Value>, task: &Map<String, Value>, os: Os) -> Map<String, Value> {
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

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawTask {
    label: String,
    detail: Option<String>,
    r#type: Option<String>,
    command: Option<Quoted>,
    args: Vec<Quoted>,
    options: TaskOptions,
    presentation: Presentation,
    group: Option<RawGroup>,
    depends_on: Option<DependsOn>,
    depends_order: Option<String>,
    is_background: bool,
    run_options: RunOptions,
    hide: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RunOptions {
    run_on: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawGroup {
    Kind(String),
    Full {
        kind: String,
        #[serde(default, rename = "isDefault")]
        is_default: bool,
    },
}

/// A label, or a task identifier object (unsupported).
#[derive(Deserialize)]
#[serde(untagged)]
enum Dep {
    Label(String),
    Identifier(IgnoredAny),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DependsOn {
    Many(Vec<Dep>),
    One(Dep),
}

impl RawTask {
    fn into_def(self) -> Result<TaskDef, String> {
        if self.label.is_empty() {
            return Err("missing label".into());
        }
        let mut unsupported = None;
        let kind = match self.r#type.as_deref().unwrap_or("shell") {
            "shell" => TaskKind::Shell,
            "process" => TaskKind::Process,
            other => {
                unsupported = Some(format!(
                    "type {other:?} comes from a VS Code extension; Xenon runs shell and process tasks"
                ));
                TaskKind::Shell
            }
        };
        let command = self.command.filter(|c| !c.value.is_empty());
        let deps = match self.depends_on {
            None => Vec::new(),
            Some(DependsOn::Many(many)) => many,
            Some(DependsOn::One(one)) => vec![one],
        };
        let mut depends_on = Vec::new();
        for dep in deps {
            match dep {
                Dep::Label(label) => depends_on.push(label),
                Dep::Identifier(_) if unsupported.is_none() => {
                    unsupported = Some(
                        "dependsOn by task identifier ({type, ...}) is not supported; use labels"
                            .into(),
                    );
                }
                Dep::Identifier(_) => {}
            }
        }
        if unsupported.is_none() && command.is_none() && depends_on.is_empty() {
            unsupported = Some("no command".into());
        }
        let group = self.group.and_then(|g| {
            let (kind, is_default) = match g {
                RawGroup::Kind(kind) => (kind, false),
                RawGroup::Full { kind, is_default } => (kind, is_default),
            };
            let kind = match kind.as_str() {
                "build" => GroupKind::Build,
                "test" => GroupKind::Test,
                _ => return None,
            };
            Some(Group { kind, is_default })
        });
        Ok(TaskDef {
            label: self.label,
            detail: self.detail,
            kind,
            command,
            args: self.args,
            options: self.options,
            presentation: self.presentation,
            group,
            depends_on,
            depends_sequence: self.depends_order.as_deref() == Some("sequence"),
            is_background: self.is_background,
            run_on_folder_open: self.run_options.run_on.as_deref() == Some("folderOpen"),
            hide: self.hide,
            unsupported,
        })
    }
}

#[derive(Deserialize)]
struct RawInput {
    id: String,
    #[serde(default)]
    description: String,
    default: Option<String>,
    r#type: Option<String>,
    #[serde(default)]
    password: bool,
    #[serde(default)]
    options: Vec<PickOption>,
}

impl RawInput {
    fn into_def(self) -> InputDef {
        let kind = match self.r#type.as_deref() {
            Some("promptString") => InputKind::PromptString {
                password: self.password,
            },
            Some("pickString") => InputKind::PickString {
                options: self.options,
            },
            _ => InputKind::Command,
        };
        InputDef {
            id: self.id,
            description: self.description,
            default: self.default,
            kind,
        }
    }
}
