use serde::{Deserialize, Serialize};

use crate::content::ContentLayout;

/// Zero-based cursor position in characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SessionState {
    pub content: ContentLayout,
}

#[derive(Deserialize)]
struct SessionStateDe {
    #[serde(default)]
    content: ContentLayout,
}

impl<'de> Deserialize<'de> for SessionState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut content = SessionStateDe::deserialize(deserializer)?.content;
        content.normalize();
        Ok(SessionState { content })
    }
}
