use super::remote::Remote;
use super::ssh_input::{SshInput, open_address};
use super::*;

impl WorkspacePickerView {
    /// The typed folder as a row above the list, when Enter would open it: a whole
    /// folder is typed (`ssh://box/src/`), or the host has nothing to offer.
    pub(super) fn ssh_row(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let SshInput::Path { host, rest } = SshInput::parse(&self.query)? else {
            return None;
        };
        let waiting = matches!(self.remote, Remote::Searching | Remote::Failed(_));
        if !self.typed_folder_selected() && !(self.results.is_empty() && !waiting) {
            return None;
        }
        let (title, subtitle, enabled) =
            match xenon_ssh::SshWorkspace::parse(&open_address(host, rest)) {
                Ok(ssh) => (format!("Connect to {}", ssh.host), ssh.directory, true),
                Err(error) => ("SSH workspace".into(), error.to_string(), false),
            };
        Some(
            query_row(
                "ssh-connect",
                QueryRow {
                    title,
                    subtitle: Some(subtitle),
                    detail: Some("SSH".into()),
                    selected: true,
                    enabled,
                    hits: Vec::new(),
                },
                cx,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.moved = false;
                this.confirm(cx)
            }))
            .into_any_element(),
        )
    }
}
