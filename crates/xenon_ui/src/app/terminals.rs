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
        let Some((ws, tab)) = self.locate_terminal(view) else {
            return;
        };
        let term = view.read(cx);
        if !term.is_exited() || term.auto_close_token() != token {
            return;
        }
        if term.auto_close().delay().is_none() {
            return;
        }
        self.drop_tab(ws, tab, None, cx);
        cx.notify();
    }
}
