//! `ssh://` queries: host rows, remote folder rows, Tab completion, status.

use super::remote::{Discovery, Remote, Reply, ReplyBody, lookup_line, merge_path};
use super::ssh_input::{
    SshInput, folder_address, host_completion, host_matches, last_segment, lookups,
    names_whole_folder, open_address, path_completion, scheme_completion,
};
use super::*;

const REMOTE_DEBOUNCE: Duration = Duration::from_millis(150);

impl WorkspacePickerView {
    /// `ssh://thin`: hosts whose names start with what was typed.
    pub(super) fn host_choices(&mut self, prefix: &str) -> Vec<WorkspaceCandidate> {
        let mut rows: Vec<WorkspaceCandidate> = self
            .known
            .iter()
            .filter(|c| matches!(c, WorkspaceCandidate::Host { name, .. } if host_matches(name, prefix)))
            .cloned()
            .collect();
        sort_candidates(&mut rows, None);
        rows
    }

    /// `ssh://host/rest`: past workspaces on the host, then what it reported.
    pub(super) fn path_results(&mut self, host: &str, rest: &str) -> Vec<WorkspaceCandidate> {
        let recents = self.host_recents(host, last_segment(rest));
        let listing = match &self.remote {
            Remote::Ready(listing) => Some(listing),
            _ => None,
        };
        merge_path(host, recents, listing)
    }

    /// Past SSH workspaces on `host`; before the host answers, only fuzzy matches.
    fn host_recents(&mut self, host: &str, needle: &str) -> Vec<(WorkspaceCandidate, String)> {
        let mine: Vec<(WorkspaceCandidate, String)> = self
            .known
            .iter()
            .filter_map(|c| {
                let ssh = self.ssh_known.get(c.root())?;
                (ssh.host == host).then(|| (c.clone(), ssh.directory.clone()))
            })
            .collect();
        if needle.is_empty() || self.remote.complete() {
            return mine;
        }
        let haystacks: Vec<String> = mine
            .iter()
            .map(|(c, dir)| format!("{} {dir}", c.name()))
            .collect();
        crate::palette::fuzzy_index_order(&haystacks, needle, &mut self.matcher)
            .into_iter()
            .map(|i| mine[i].clone())
            .collect()
    }

    /// Ask the host about the current query; later answers replace earlier ones.
    pub(super) fn kick_remote(&mut self, cx: &mut Context<Self>) {
        self._discover_task = None;
        self.discover_gen = self.discover_gen.wrapping_add(1);
        let query = self.query.clone();
        let Some(SshInput::Path { host, rest }) = SshInput::parse(&query) else {
            self.remote = Remote::Idle;
            return;
        };
        self.remote = Remote::Searching;
        let (token, host) = (self.discover_gen, host.to_string());
        let (listing, search) = lookups(rest);
        self._discover_task = Some(cx.spawn(async move |this, cx| {
            let background = cx.background_executor().clone();
            background.timer(REMOTE_DEBOUNCE).await;
            let searching = search.is_some();
            let listing_host = host.clone();
            let listed = background.spawn(async move { lookup_line(&listing_host, &listing) });
            let found = search.map(|q| background.spawn(async move { lookup_line(&host, &q) }));
            let listing = listed.await;
            let failed = listing.is_err();
            let body = ReplyBody::Listed { listing, searching };
            this.update(cx, |this, cx| this.apply_remote(Reply { token, body }, cx))
                .ok();
            let Some(found) = found.filter(|_| !failed) else {
                return;
            };
            let body = ReplyBody::Discovered(found.await);
            this.update(cx, |this, cx| this.apply_remote(Reply { token, body }, cx))
                .ok();
        }));
    }

    fn apply_remote(&mut self, reply: Reply, cx: &mut Context<Self>) {
        let remote = std::mem::replace(&mut self.remote, Remote::Idle);
        self.remote = remote.apply(self.discover_gen, reply);
        self.refilter();
        cx.notify();
    }

    /// Put `text` in the field and treat it as typed.
    fn write_query(&mut self, text: String, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_text(text.clone(), cx));
        self.set_query(text, cx);
    }

    /// Enter on a host row writes `ssh://host/`; elsewhere it falls through.
    pub(super) fn confirm_host_row(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(WorkspaceCandidate::Host { name, .. }) = self.results.get(self.selected) else {
            return false;
        };
        let text = host_completion(name);
        self.write_query(text, cx);
        true
    }

    /// Enter inside `ssh://host/rest`. A typed whole folder (`ssh://box/src/`)
    /// opens unless the user moved to a row; otherwise the selected row opens.
    pub(super) fn confirm_remote(&mut self, host: &str, rest: &str, cx: &mut Context<Self>) {
        let typed_folder = !self.moved && names_whole_folder(rest);
        let row = self.results.get(self.selected).cloned();
        match row {
            Some(WorkspaceCandidate::Remote { host, root, .. }) if !typed_folder => {
                let address = folder_address(&host, &root.to_string_lossy());
                cx.emit(WorkspacePickerEvent::Ssh(address));
            }
            Some(row) if !typed_folder && row.selectable() => {
                cx.emit(WorkspacePickerEvent::Open(row));
            }
            _ => cx.emit(WorkspacePickerEvent::Ssh(open_address(host, rest))),
        }
    }

    /// Tab: `ss` becomes `ssh://`; a selected host or folder is written into the field.
    pub(super) fn complete_selected(&mut self, cx: &mut Context<Self>) {
        let text = scheme_completion(&self.query)
            .map(str::to_string)
            .or_else(|| {
                self.results
                    .get(self.selected)
                    .and_then(|c| self.completion_for(c))
            });
        if let Some(text) = text {
            self.write_query(text, cx);
        }
    }

    fn completion_for(&self, row: &WorkspaceCandidate) -> Option<String> {
        if let WorkspaceCandidate::Host { name, .. } = row {
            return Some(host_completion(name));
        }
        let SshInput::Path { host, rest } = SshInput::parse(&self.query)? else {
            return None;
        };
        let path = match row {
            WorkspaceCandidate::Remote { root, .. } => root.to_string_lossy().into_owned(),
            _ => self.ssh_known.get(row.root())?.directory.clone(),
        };
        Some(path_completion(host, rest, &path, self.remote.home()))
    }

    /// The typed folder (`ssh://box/src/`) is what Enter opens right now.
    pub(super) fn typed_folder_selected(&self) -> bool {
        match SshInput::parse(&self.query) {
            Some(SshInput::Path { rest, .. }) => !self.moved && names_whole_folder(rest),
            _ => false,
        }
    }

    pub(super) fn status_row(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let SshInput::Path { host, .. } = SshInput::parse(&self.query)? else {
            return None;
        };
        let (title, subtitle) = match &self.remote {
            Remote::Searching => (format!("Searching {host}…"), None),
            Remote::Failed(error) => (
                format!("Couldn’t reach {host}"),
                Some(format!("Run `ssh {host}` once in a terminal · {error}")),
            ),
            Remote::Ready(listing) => match &listing.discovery {
                Discovery::Pending => (format!("Searching {host}…"), None),
                Discovery::Failed(error) => {
                    (format!("Couldn’t search {host}"), Some(error.clone()))
                }
                Discovery::Done => return None,
            },
            Remote::Idle => return None,
        };
        let row = QueryRow {
            title,
            subtitle,
            detail: None,
            selected: false,
            enabled: false,
            hits: Vec::new(),
        };
        Some(query_row("host-status", row, cx).into_any_element())
    }

    pub(super) fn subtitle_of(&self, cand: &WorkspaceCandidate) -> Option<String> {
        match self.ssh_known.get(cand.root()) {
            Some(ssh) => Some(format!("{}:{}", ssh.host, ssh.directory)),
            None => cand.subtitle(),
        }
    }

    /// What the typed text is looking for, for highlighting row titles.
    pub(super) fn hit_needle(&self) -> &str {
        match SshInput::parse(&self.query) {
            Some(SshInput::Path { rest, .. }) => last_segment(rest),
            Some(SshInput::Hosts(_)) | None => &self.query,
        }
    }

    pub(super) fn empty_message(&self) -> String {
        match SshInput::parse(&self.query) {
            Some(SshInput::Hosts(_)) => "No SSH host matches — keep typing ssh://host/path".into(),
            Some(SshInput::Path { host, .. }) => format!("No folders match on {host}"),
            None if self.query.is_empty() => {
                "No workspaces — type a path, name, or ~/src name".into()
            }
            None => "No match — try ~/src name or Browse…".into(),
        }
    }

    pub(super) fn hint(&self) -> String {
        match SshInput::parse(&self.query) {
            Some(_) => "return opens  ·  tab completes  ·  esc closes".into(),
            None => "return opens  ·  ⌘⌫ forgets a closed workspace  ·  esc closes".into(),
        }
    }
}

#[cfg(feature = "visual-tests")]
impl WorkspacePickerView {
    pub(crate) fn visual_hosts(&mut self, hosts: &[&str], cx: &mut Context<Self>) {
        let names = hosts.iter().map(|h| (*h).to_string()).collect();
        self.known.extend(remote::host_candidates(names, &[]));
        self.refilter();
        cx.notify();
    }

    pub(crate) fn visual_remote(&mut self, query: &str, remote: Remote, cx: &mut Context<Self>) {
        self.remote = remote;
        self.visual_query(query, cx);
    }
}
