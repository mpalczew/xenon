use super::model::{InputDef, InputKind, PickOption};

#[derive(Clone, Debug)]
pub struct TaskQuestion {
    pub id: String,
    pub description: String,
    pub default: Option<String>,
    pub kind: QuestionKind,
}

#[derive(Clone, Debug)]
pub enum QuestionKind {
    Text { password: bool },
    Pick { options: Vec<PickOption> },
}

impl TaskQuestion {
    /// `None` for a `command` input, which cannot be asked.
    pub fn of(def: &InputDef) -> Option<Self> {
        let kind = match &def.kind {
            InputKind::PickString { options } => QuestionKind::Pick {
                options: options.clone(),
            },
            InputKind::PromptString { password } => QuestionKind::Text {
                password: *password,
            },
            InputKind::Command => return None,
        };
        Some(Self {
            id: def.id.clone(),
            description: def.description.clone(),
            default: def.default.clone(),
            kind,
        })
    }

    pub fn prompt_text(&self) -> &str {
        if self.description.is_empty() {
            &self.id
        } else {
            &self.description
        }
    }

    pub fn placeholder(&self) -> &'static str {
        match self.kind {
            QuestionKind::Text { .. } => "Value…",
            QuestionKind::Pick { .. } => "Filter…",
        }
    }

    pub fn hint(&self) -> &'static str {
        match &self.kind {
            QuestionKind::Text { password: true } => {
                "return confirms  ·  esc cancels  ·  hidden, never saved or echoed"
            }
            QuestionKind::Text { .. } => "return confirms  ·  esc cancels",
            QuestionKind::Pick { .. } => "↑↓ choose  ·  return confirms  ·  esc cancels",
        }
    }
}
