use super::ssh_input::{SshInput, open_address, scheme_prefix};
use super::*;

impl WorkspacePickerView {
    /// `ssh://` as a completion row while the query is a prefix of it. Ranked by
    /// the latest SSH use, so recent SSH work wins over an older local match.
    pub(super) fn scheme_row(&self) -> Option<WorkspaceCandidate> {
        scheme_prefix(&self.query).then(|| WorkspaceCandidate::Scheme {
            last_opened: self.last_ssh_use(),
            leads: self.query.trim_start().len() >= 3,
        })
    }

    fn last_ssh_use(&self) -> u64 {
        self.known
            .iter()
            .filter(|c| {
                matches!(c, WorkspaceCandidate::Host { .. })
                    || self.ssh_known.contains_key(c.root())
            })
            .map(WorkspaceCandidate::last_opened)
            .max()
            .unwrap_or(0)
    }

    /// The typed folder as a row above the list, when Enter would open it: a whole
    /// folder is typed (`ssh://box/src/`), or the host has nothing to offer.
    pub(super) fn ssh_row(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let SshInput::Path { host, rest } = SshInput::parse(&self.query)? else {
            return None;
        };
        let waiting = self.remote.waiting();
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
