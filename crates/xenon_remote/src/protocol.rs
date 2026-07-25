//! JSON wire types for mobile remote (HTTP lean stack).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    /// Live in this app process (terminals available).
    #[serde(default)]
    pub open: bool,
    /// Root path for display (recent / closed picker).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub root: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectRequest {
    pub workspace_id: String,
    pub tab_id: u64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInfo {
    pub tab_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub cwd: String,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRequest {
    pub token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub ticket: String,
}

/// JSON metadata for a frame when client wants headers-only / debugging.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameMeta {
    pub tab_id: u64,
    pub seq: u64,
    pub cols: u16,
    pub rows: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_roundtrip() {
        let j = r#"{"workspaceId":"w","tabId":3,"text":"hi\n"}"#;
        let r: InjectRequest = serde_json::from_str(j).unwrap();
        assert_eq!(r.tab_id, 3);
        assert_eq!(r.text, "hi\n");
    }
}
