use super::{RemoteFile, RemoteRead, RemoteSnapshot};
use anyhow::Result;
use serde_json::json;

impl RemoteFile {
    pub fn read(&self, revision: Option<&str>) -> Result<RemoteRead> {
        Ok(serde_json::from_value(self.workspace.request(
            json!({"op": "read", "path": self.relative, "revision": revision}),
        )?)?)
    }

    /// Compare before writing; a conflict is distinct from a transport failure.
    pub fn write(&self, text: &str, expected: Option<&str>) -> Result<RemoteSnapshot> {
        Ok(serde_json::from_value(self.workspace.request(json!({
            "op": "write", "path": self.relative, "text": text, "expected": expected
        }))?)?)
    }
}
