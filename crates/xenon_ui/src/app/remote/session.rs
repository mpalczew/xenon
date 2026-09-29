//! One phone connection: attach to a terminal, stream frames on change,
//! forward input, apply phone fit, and push status dots.

use super::*;
use xenon_remote::{
    ClientMsg, ClosedReason, FrameEncoder, NamedKey, Outbox, Rgb, ServerMsg, ThemeWire,
};
use xenon_terminal::PhoneFit;

use super::devices::device_short_name;
use super::wire::wire_screen;

/// Frame coalescing window (≤ 30 fps while output streams).
const FLUSH_AFTER: Duration = Duration::from_millis(33);
/// Retry after a full outbox (slow phone link).
const RETRY_AFTER: Duration = Duration::from_millis(250);
/// Phone fit bounds (anything else is a buggy or hostile client).
const FIT_COLS: std::ops::RangeInclusive<u16> = 20..=300;
const FIT_ROWS: std::ops::RangeInclusive<u16> = 8..=200;

pub(super) struct Conn {
    pub device_id: String,
    pub(super) out: Outbox,
    pub(super) attach: Option<Attach>,
    pub(super) last_dots: Option<ServerMsg>,
}

pub(super) struct Attach {
    pub(super) workspace: WorkspaceId,
    tab: TabId,
    pub(super) view: WeakEntity<TerminalView>,
    encoder: FrameEncoder,
    fit: Option<(u16, u16)>,
    flush: Option<Task<()>>,
    _observe: Subscription,
    /// The tab closed on the Mac: tell the phone.
    _release: Subscription,
}

impl Conn {
    #[cfg(feature = "visual-tests")]
    pub(super) fn visual(device_id: &str, out: Outbox) -> Self {
        Self {
            device_id: device_id.into(),
            out,
            attach: None,
            last_dots: None,
        }
    }
}

impl XenonApp {
    /// Visual: the connected phone drives the active terminal at 45×30.
    #[cfg(feature = "visual-tests")]
    pub(crate) fn visual_phone_driving(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.active else {
            return;
        };
        let Some((tab, view)) = self.active_content().and_then(|c| {
            let leaf = c.focused_leaf()?;
            match leaf.active_tab()? {
                LiveTab::Terminal { id, view } => Some((*id, view.clone())),
                _ => None,
            }
        }) else {
            return;
        };
        let observe = cx.observe(&view, |_, _, _| {});
        let release = cx.observe_release(&view, |_, _, _| {});
        if let Some(c) = self.conn_mut(ConnId(1)) {
            c.attach = Some(Attach {
                workspace,
                tab,
                view: view.downgrade(),
                encoder: FrameEncoder::default(),
                fit: Some((45, 30)),
                flush: None,
                _observe: observe,
                _release: release,
            });
        }
        self.apply_tab_fit(workspace, tab, cx);
        self.publish_remote_info(cx);
    }

    pub(super) fn remote_connected(
        &mut self,
        conn: ConnId,
        device_id: String,
        out: Outbox,
        cx: &mut Context<Self>,
    ) {
        let [bg, fg, cursor] = xenon_terminal::remote_theme_colors(cx);
        let Some(runtime) = self.services.remote.as_mut() else {
            return;
        };
        out.try_send(ServerMsg::Ready {
            theme: ThemeWire {
                bg: Rgb(bg[0], bg[1], bg[2]).hex(),
                fg: Rgb(fg[0], fg[1], fg[2]).hex(),
                cursor: Rgb(cursor[0], cursor[1], cursor[2]).hex(),
            },
            device_id: device_id.clone(),
            host_name: runtime.host_name.clone(),
        });
        runtime.conns.insert(
            conn,
            Conn {
                device_id,
                out,
                attach: None,
                last_dots: None,
            },
        );
        self.broadcast_remote_dots(cx);
        self.publish_remote_info(cx);
    }

    pub(super) fn remote_disconnected(&mut self, conn: ConnId, cx: &mut Context<Self>) {
        self.remote_detach(conn, cx);
        if let Some(runtime) = self.services.remote.as_mut() {
            runtime.conns.remove(&conn);
        }
        self.publish_remote_info(cx);
    }

    pub(super) fn remote_message(
        &mut self,
        conn: ConnId,
        msg: ClientMsg,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match msg {
            ClientMsg::Attach {
                workspace_id,
                tab_id,
            } => {
                let target = host::parse_workspace_id(&workspace_id)
                    .ok()
                    .map(|wid| (wid, TabId(tab_id)));
                self.remote_attach(conn, target, window, cx);
            }
            ClientMsg::Detach => self.remote_detach(conn, cx),
            ClientMsg::Text { data } => {
                if let Some(view) = self.attached_view(conn) {
                    view.update(cx, |term, cx| term.inject_text(&data, cx));
                }
            }
            ClientMsg::Key { key } => self.remote_key(conn, key, cx),
            ClientMsg::History => self.remote_history(conn, cx),
            ClientMsg::Fit { cols, rows } => self.remote_fit(conn, cols, rows, cx),
            ClientMsg::Hello { .. } | ClientMsg::Ping => {}
        }
    }

    fn remote_attach(
        &mut self,
        conn: ConnId,
        target: Option<(WorkspaceId, TabId)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.remote_detach(conn, cx);
        let tab = target.map_or(TabId(0), |(_, tab)| tab);
        let view = target
            .filter(|(wid, _)| self.ensure_workspace_live(*wid, cx).is_ok())
            .and_then(|(wid, tab)| Some((wid, self.find_terminal_view(wid, tab)?.clone())));
        let Some((workspace, view)) = view else {
            self.send_to(
                conn,
                ServerMsg::Closed {
                    reason: ClosedReason::TabClosed,
                },
            );
            return;
        };
        self.ensure_remote_terminal_sizes(workspace, cx);
        let observe = cx.observe(&view, move |app, _, cx| app.schedule_remote_flush(conn, cx));
        let release = cx.observe_release(&view, move |app, _, cx| app.remote_tab_gone(conn, cx));
        if let Some(c) = self.conn_mut(conn) {
            c.attach = Some(Attach {
                workspace,
                tab,
                view: view.downgrade(),
                encoder: FrameEncoder::default(),
                fit: None,
                flush: None,
                _observe: observe,
                _release: release,
            });
        }
        self.flush_remote(conn, window, cx);
        self.broadcast_remote_dots(cx);
    }

    /// The attached tab closed or its shell exited: release it and tell the phone.
    fn remote_tab_gone(&mut self, conn: ConnId, cx: &mut Context<Self>) {
        self.remote_detach(conn, cx);
        self.send_to(
            conn,
            ServerMsg::Closed {
                reason: ClosedReason::TabClosed,
            },
        );
    }

    fn remote_detach(&mut self, conn: ConnId, cx: &mut Context<Self>) {
        let Some(attach) = self.conn_mut(conn).and_then(|c| c.attach.take()) else {
            return;
        };
        self.apply_tab_fit(attach.workspace, attach.tab, cx);
        self.publish_remote_info(cx);
    }

    fn remote_key(&mut self, conn: ConnId, key: NamedKey, cx: &mut Context<Self>) {
        if let Some(view) = self.attached_view(conn) {
            view.update(cx, |term, cx| term.send_named_key(key.keystroke(), cx));
        }
    }

    fn remote_history(&mut self, conn: ConnId, cx: &mut Context<Self>) {
        let Some(view) = self.attached_view(conn) else {
            return;
        };
        let lines = view.read(cx).history_text(cx);
        self.send_to(conn, ServerMsg::History { lines });
    }

    fn remote_fit(&mut self, conn: ConnId, cols: u16, rows: u16, cx: &mut Context<Self>) {
        let fit = (
            cols.clamp(*FIT_COLS.start(), *FIT_COLS.end()),
            rows.clamp(*FIT_ROWS.start(), *FIT_ROWS.end()),
        );
        let Some(attach) = self.conn_mut(conn).and_then(|c| c.attach.as_mut()) else {
            return;
        };
        attach.fit = Some(fit);
        let (workspace, tab) = (attach.workspace, attach.tab);
        self.apply_tab_fit(workspace, tab, cx);
        self.publish_remote_info(cx);
    }

    /// Smallest fit among phones attached to this tab (None releases it).
    fn apply_tab_fit(&mut self, workspace: WorkspaceId, tab: TabId, cx: &mut Context<Self>) {
        let Some(runtime) = self.services.remote.as_ref() else {
            return;
        };
        let fit = runtime
            .conns
            .values()
            .filter_map(|c| {
                let a = c.attach.as_ref()?;
                (a.workspace == workspace && a.tab == tab)
                    .then_some(())
                    .and(a.fit)
                    .map(|(cols, rows)| (cols, rows, c.device_id.clone()))
            })
            // Each dimension's minimum, so every phone's screen holds the grid.
            .reduce(|(c1, r1, d1), (c2, r2, _)| (c1.min(c2), r1.min(r2), d1))
            .map(|(cols, rows, device)| PhoneFit {
                cols,
                rows,
                device: device_short_name(runtime.devices.label(&device).unwrap_or("Phone"))
                    .to_string()
                    .into(),
            });
        if let Some(view) = self.find_terminal_view(workspace, tab).cloned() {
            view.update(cx, |term, cx| term.set_phone_fit(fit, cx));
        }
    }

    /// Hand a connection's terminal back to the Mac (server stopping).
    pub(super) fn release_phone_fit(&self, conn: &Conn, cx: &mut Context<Self>) {
        if let Some(view) = conn.attach.as_ref().and_then(|a| a.view.upgrade()) {
            view.update(cx, |term, cx| term.set_phone_fit(None, cx));
        }
    }

    fn schedule_remote_flush(&mut self, conn: ConnId, cx: &mut Context<Self>) {
        self.schedule_remote_flush_after(conn, FLUSH_AFTER, cx);
    }

    fn schedule_remote_flush_after(
        &mut self,
        conn: ConnId,
        delay: Duration,
        cx: &mut Context<Self>,
    ) {
        let main = self.services.main_window;
        let Some(attach) = self.conn_mut(conn).and_then(|c| c.attach.as_mut()) else {
            return;
        };
        if attach.flush.is_some() {
            return;
        }
        attach.flush = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let Some(main) = main else { return };
            let _ = main.update(cx, |_, window, cx| {
                this.update(cx, |app, cx| {
                    if let Some(a) = app.conn_mut(conn).and_then(|c| c.attach.as_mut()) {
                        a.flush = None;
                    }
                    app.flush_remote(conn, window, cx);
                })
            });
        }));
    }

    /// Snapshot the attached terminal and send what changed.
    fn flush_remote(&mut self, conn: ConnId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.attached_view(conn) else {
            return;
        };
        if view.read(cx).is_exited() {
            self.remote_tab_gone(conn, cx);
            return;
        }
        let Some(snapshot) = view.update(cx, |term, cx| term.screen_snapshot(window, cx)) else {
            return;
        };
        let screen = wire_screen(&snapshot);
        let Some(c) = self.conn_mut(conn) else {
            return;
        };
        let Some(attach) = c.attach.as_mut() else {
            return;
        };
        let msgs = attach.encoder.encode(&screen);
        let delivered = msgs.into_iter().all(|m| c.out.try_send(m));
        if !delivered {
            attach.encoder.force_full();
            self.schedule_remote_flush_after(conn, RETRY_AFTER, cx);
        }
    }

    /// Revoke from Settings: tell the phone, then drop its connections.
    pub(super) fn drop_device_connections(
        &mut self,
        device_id: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let Some(runtime) = self.services.remote.as_mut() else {
            return;
        };
        let doomed: Vec<ConnId> = runtime
            .conns
            .iter()
            .filter(|(_, c)| device_id.is_none_or(|d| c.device_id == d))
            .map(|(id, _)| *id)
            .collect();
        for id in doomed {
            self.send_to(
                id,
                ServerMsg::Closed {
                    reason: ClosedReason::Revoked,
                },
            );
            self.remote_disconnected(id, cx);
        }
    }

    fn attached_view(&self, conn: ConnId) -> Option<Entity<TerminalView>> {
        self.conn(conn)?.attach.as_ref()?.view.upgrade()
    }

    fn send_to(&self, conn: ConnId, msg: ServerMsg) {
        if let Some(c) = self.conn(conn) {
            c.out.try_send(msg);
        }
    }

    pub(super) fn conn(&self, conn: ConnId) -> Option<&Conn> {
        self.services.remote.as_ref()?.conns.get(&conn)
    }

    pub(super) fn conn_mut(&mut self, conn: ConnId) -> Option<&mut Conn> {
        self.services.remote.as_mut()?.conns.get_mut(&conn)
    }
}
