use super::*;

impl XenonApp {
    /// Shell process died: restyle tab, then maybe auto-close per this tab's policy.
    pub(super) fn on_terminal_exited(
        &mut self,
        view: Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        self.schedule_terminal_auto_close(view, cx);
    }

    pub(super) fn on_terminal_auto_close_changed(
        &mut self,
        view: Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) {
        self.schedule_terminal_auto_close(view, cx);
    }

    fn schedule_terminal_auto_close(&mut self, view: Entity<TerminalView>, cx: &mut Context<Self>) {
        let (delay, token) = {
            let term = view.read(cx);
            if !term.is_exited() {
                return;
            }
            let Some(delay) = term.auto_close().delay() else {
                return;
            };
            (delay, term.auto_close_token())
        };
        cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            this.update(cx, |this, cx| {
                this.close_terminal_entity_if_still_due(&view, token, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn close_terminal_entity_if_still_due(
        &mut self,
        view: &Entity<TerminalView>,
        token: u64,
        cx: &mut Context<Self>,
    ) {
        let Some((ws, _pane, _idx)) = self.locate_terminal(view) else {
            return;
        };
        let term = view.read(cx);
        if !term.is_exited() || term.auto_close_token() != token {
            return;
        }
        if term.auto_close().delay().is_none() {
            return;
        }
        let tab = self.locate_terminal(view).and_then(|(ws2, pane, idx)| {
            self.contents
                .get(&ws2)
                .and_then(|c| c.root.as_ref()?.find_leaf(pane))
                .and_then(|l| l.tabs.get(idx).map(|t| t.id()))
        });
        let _ = ws;
        if let Some(tab) = tab {
            let ws = self.locate_terminal(view).map(|(w, _, _)| w).unwrap_or(ws);
            self.drop_tab(ws, tab, None, cx);
        }
        cx.notify();
    }
}
