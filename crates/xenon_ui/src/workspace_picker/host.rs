//! Inside-host mode: Enter on an SSH host searches that host's folders.

use xenon_ssh::{HostQuery, RemoteListing, tilde_path};

use super::scope::{Remote, completion, merge_remote, short_error};
use super::*;

const REMOTE_DEBOUNCE: Duration = Duration::from_millis(150);
const LOCAL_PLACEHOLDER: &str = "Folder or ssh://host/path…";

impl WorkspacePickerView {
    /// Past workspaces on this host first, then folders the host reported.
    pub(super) fn host_results(&mut self) -> Vec<WorkspaceCandidate> {
        let Some(host) = self.scope.host().cloned() else {
            return Vec::new();
        };
        let query = HostQuery::parse(&self.query);
        let recents = self.host_recents(&host.name, &query);
        let listing = match &host.remote {
            Remote::Ready(listing) => Some(listing),
            _ => None,
        };
        let mut results = merge_remote(&host.name, recents, &query, listing);
        match query.needle() {
            Some(needle) => sort_candidates(&mut results, Some(needle)),
            None if query == HostQuery::Empty => sort_candidates(&mut results, None),
            None => sort_listing(&mut results),
        }
        results
    }

    fn host_recents(&mut self, host: &str, query: &HostQuery) -> Vec<(WorkspaceCandidate, String)> {
        let mine: Vec<(WorkspaceCandidate, String)> = self
            .known
            .iter()
            .filter_map(|c| {
                let ssh = self.ssh_known.get(c.root())?;
                (ssh.host == host).then(|| (c.clone(), ssh.directory.clone()))
            })
            .collect();
        let Some(needle) = query.needle() else {
            return mine;
        };
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
        let Scope::Host(host) = &mut self.scope else {
            return;
        };
        self._discover_task = None;
        self.discover_gen = self.discover_gen.wrapping_add(1);
        let query = HostQuery::parse(&self.query);
        if query == HostQuery::Empty {
            host.remote = Remote::Idle;
            return;
        }
        host.remote = Remote::Searching;
        let (token, name) = (self.discover_gen, host.name.clone());
        self._discover_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REMOTE_DEBOUNCE).await;
            let host = name.clone();
            let result = cx
                .background_executor()
                .spawn(async move { xenon_ssh::lookup_host(&host, &query) })
                .await;
            this.update(cx, |this, cx| {
                if this.discover_gen == token {
                    this.apply_remote(result, cx);
                }
            })
            .ok();
        }));
    }

    fn apply_remote(
        &mut self,
        result: anyhow::Result<Option<RemoteListing>>,
        cx: &mut Context<Self>,
    ) {
        let Scope::Host(host) = &mut self.scope else {
            return;
        };
        host.remote = match result {
            Ok(Some(listing)) => Remote::Ready(listing),
            Ok(None) => Remote::Idle,
            Err(error) => Remote::Failed(short_error(&error.to_string())),
        };
        self.refilter();
        cx.notify();
    }

    pub(super) fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        let placeholder = match scope.host() {
            Some(host) => format!("Search folders on {}…", host.name),
            None => LOCAL_PLACEHOLDER.to_string(),
        };
        self.scope = scope;
        self.query.clear();
        self.discovered.clear();
        self._discover_task = None;
        self.discover_gen = self.discover_gen.wrapping_add(1);
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, cx);
            input.set_text("", cx);
        });
        self.refilter();
        cx.notify();
    }

    pub(super) fn leave_host_if_empty(&mut self, cx: &mut Context<Self>) {
        if let Some(scope) = self.scope.after_backspace(self.query.is_empty()) {
            self.set_scope(scope, cx);
        }
    }

    /// Enter on a host steps inside it; on a remote folder it opens that folder.
    pub(super) fn confirm_host_row(&mut self, cx: &mut Context<Self>) -> bool {
        match self.results.get(self.selected).cloned() {
            Some(WorkspaceCandidate::Host { name, .. }) => {
                self.set_scope(Scope::enter(name), cx);
                true
            }
            Some(WorkspaceCandidate::Remote { host, root, .. }) => {
                let address = format!("ssh://{host}{}", root.display());
                cx.emit(WorkspacePickerEvent::Ssh(address));
                true
            }
            _ => false,
        }
    }

    /// Tab: step into a host, or write the selected folder into the field.
    pub(super) fn complete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(selected) = self.results.get(self.selected).cloned() else {
            return;
        };
        let path = match (&self.scope, &selected) {
            (Scope::Local, WorkspaceCandidate::Host { .. }) => {
                self.confirm_host_row(cx);
                return;
            }
            (Scope::Host(_), WorkspaceCandidate::Remote { display, .. }) => display.clone(),
            (Scope::Host(host), _) => match self.ssh_known.get(selected.root()) {
                Some(ssh) => match &host.remote {
                    Remote::Ready(listing) => tilde_path(&ssh.directory, &listing.home),
                    _ => ssh.directory.clone(),
                },
                None => return,
            },
            _ => return,
        };
        let text = completion(&path);
        self.input
            .update(cx, |input, cx| input.set_text(text.clone(), cx));
        self.set_query(text, cx);
    }

    pub(super) fn status_row(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let host = self.scope.host()?;
        let (title, subtitle) = match &host.remote {
            Remote::Searching => (format!("Searching {}…", host.name), None),
            Remote::Failed(error) => (
                format!("Couldn’t reach {}", host.name),
                Some(format!(
                    "Run `ssh {}` once in a terminal · {error}",
                    host.name
                )),
            ),
            Remote::Idle | Remote::Ready(_) => return None,
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

    pub(super) fn empty_message(&self) -> String {
        match self.scope.host() {
            Some(host) if self.query.is_empty() => {
                format!("Type a folder name or a path like ~/src/ on {}", host.name)
            }
            Some(host) => format!("No folders match on {}", host.name),
            None if self.query.is_empty() => {
                "No workspaces — type a path, name, or ~/src name".into()
            }
            None => "No match — try ~/src name or Browse…".into(),
        }
    }

    pub(super) fn hint(&self) -> String {
        match self.scope.host() {
            Some(host) => format!(
                "on {}  ·  return opens  ·  tab completes  ·  ⌫ leaves host  ·  esc closes",
                host.name
            ),
            None => "return opens  ·  ⌘⌫ forgets a closed workspace  ·  esc closes".into(),
        }
    }
}

#[cfg(feature = "visual-tests")]
impl WorkspacePickerView {
    pub(crate) fn visual_hosts(&mut self, hosts: &[&str], cx: &mut Context<Self>) {
        let names = hosts.iter().map(|h| (*h).to_string()).collect();
        self.known.extend(scope::host_candidates(names, &[]));
        self.refilter();
        cx.notify();
    }

    pub(crate) fn visual_in_host(
        &mut self,
        host: &str,
        query: &str,
        remote: Remote,
        cx: &mut Context<Self>,
    ) {
        self.set_scope(Scope::enter(host.into()), cx);
        self.query = query.into();
        self.input.update(cx, |input, cx| input.set_text(query, cx));
        if let Scope::Host(scope) = &mut self.scope {
            scope.remote = remote;
        }
        self.refilter();
        cx.notify();
    }
}
