use crate::ipc::OpenFileSpec;

impl OpenFileSpec {
    /// Zero-based (row, column) of the 1-based `line`/`column`, if a line is given.
    pub fn cursor(&self) -> Option<(u32, u32)> {
        self.line.map(|line| {
            (
                line.saturating_sub(1),
                self.column.unwrap_or(1).saturating_sub(1),
            )
        })
    }
}
