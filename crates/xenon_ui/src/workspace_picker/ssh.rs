use super::*;

impl WorkspacePickerView {
    pub(super) fn ssh_row(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if !self.query.starts_with("ssh://") {
            return None;
        }
        let (title, subtitle, enabled) = match xenon_ssh::SshWorkspace::parse(&self.query) {
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
            .on_click(cx.listener(|this, _, _, cx| this.confirm(cx)))
            .into_any_element(),
        )
    }
}
