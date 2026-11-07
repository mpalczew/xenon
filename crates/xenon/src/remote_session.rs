//! One phone connection: attach to a terminal, forward input, scroll, fit,
//! and keep the phone's status dots current.

use std::collections::BTreeMap;
use std::rc::Rc;

use gtk::prelude::*;
use vte::prelude::*;
use xenon_core::{TabId, WorkspaceId};
use xenon_remote::{
    AppearanceWire, ClientMsg, ClosedReason, ConnId, Dot, InputRoute, Outbox, Rgb, ServerMsg,
    clamp_fit, parse_workspace_id, pty_text_bytes, wire_dot,
};

use crate::app::App;
use crate::remote_mirror::Mirror;

pub(crate) struct Conn {
    pub device_id: String,
    pub out: Outbox,
    pub attach: Option<Attach>,
    /// The phone's own light or dark choice; `None` follows the Mac.
    appearance: Option<AppearanceWire>,
    low_bandwidth: bool,
    last_dots: Option<ServerMsg>,
}

pub(crate) struct Attach {
    pub ws: WorkspaceId,
    pub tab: TabId,
    pub fit: Option<(u16, u16)>,
    pub mirror: Mirror,
}

/// Scrollback lines sent to a phone.
const HISTORY_LIMIT: i64 = 1000;

fn rgb(hex: &str) -> [u8; 3] {
    let c = gtk::gdk::RGBA::parse(hex).unwrap_or(gtk::gdk::RGBA::BLACK);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(c.red()), byte(c.green()), byte(c.blue())]
}

impl App {
    /// Terminal default background, foreground and cursor for the page, in
    /// the phone's chosen look. The ANSI palette is the same in both looks, so
    /// only the defaults differ and cell colors from VTE still match.
    fn remote_colors(appearance: Option<AppearanceWire>) -> [[u8; 3]; 3] {
        let dark = match appearance {
            Some(look) => look == AppearanceWire::Dark,
            None => crate::theme::dark(),
        };
        let (fg, bg, _) = crate::theme::terminal(dark);
        [rgb(bg), rgb(fg), rgb(fg)]
    }

    /// The Mac terminal's own default foreground and background: the colors
    /// VTE's text dump leaves out, which the phone shows as its defaults.
    fn remote_defaults() -> (Rgb, Rgb) {
        let [bg, fg, _] = Self::remote_colors(None);
        (Rgb(fg[0], fg[1], fg[2]), Rgb(bg[0], bg[1], bg[2]))
    }

    pub(crate) fn remote_connected(
        self: &Rc<Self>,
        conn: ConnId,
        device_id: String,
        out: Outbox,
        _input: InputRoute,
    ) {
        let theme = xenon_remote::theme_wire(Self::remote_colors(None));
        {
            let mut rt = self.remote.rt.borrow_mut();
            let Some(rt) = rt.as_mut() else { return };
            out.try_send(ServerMsg::Ready {
                theme,
                device_id: device_id.clone(),
                host_name: rt.host_name.clone(),
            });
            rt.conns.insert(
                conn,
                Conn {
                    device_id,
                    out,
                    attach: None,
                    appearance: None,
                    low_bandwidth: false,
                    last_dots: None,
                },
            );
        }
        self.remote_dots();
        self.remote_publish();
    }

    pub(crate) fn remote_disconnected(self: &Rc<Self>, conn: ConnId) {
        self.remote_detach(conn);
        self.remote_task_gone(conn);
        if let Some(rt) = self.remote.rt.borrow_mut().as_mut() {
            rt.conns.remove(&conn);
        }
        self.remote_publish();
    }

    pub(crate) fn remote_message(self: &Rc<Self>, conn: ConnId, msg: ClientMsg) {
        match msg {
            ClientMsg::Attach {
                workspace_id,
                tab_id,
            } => {
                let target = parse_workspace_id(&workspace_id).ok();
                self.remote_attach(conn, target.map(|ws| (ws, TabId(tab_id))));
            }
            ClientMsg::Bandwidth { low_bandwidth } => {
                self.with_conn(conn, |c| {
                    c.low_bandwidth = low_bandwidth;
                    if let Some(a) = c.attach.as_ref() {
                        a.mirror.set_low_bandwidth(low_bandwidth);
                    }
                });
            }
            ClientMsg::Appearance { appearance } => {
                self.with_conn(conn, |c| c.appearance = appearance);
                let theme = xenon_remote::theme_wire(Self::remote_colors(appearance));
                self.send_to(conn, ServerMsg::Theme { theme });
            }
            ClientMsg::Detach => self.remote_detach(conn),
            ClientMsg::Text { data } => self.remote_write(conn, &pty_text_bytes(&data)),
            ClientMsg::Key { key } => self.remote_write(conn, key.pty_bytes(false)),
            ClientMsg::History => self.remote_history(conn),
            ClientMsg::Wheel { rows } => self.remote_scroll(conn, Some(rows.clamp(-40, 40))),
            ClientMsg::Bottom => self.remote_scroll(conn, None),
            ClientMsg::Fit { cols, rows } => {
                let fit = clamp_fit(cols, rows);
                let target = self.with_conn(conn, |c| {
                    c.attach.as_mut().map(|a| {
                        a.fit = Some(fit);
                        (a.ws, a.tab)
                    })
                });
                if let Some((ws, tab)) = target.flatten() {
                    self.remote_tab_fit(ws, tab);
                    self.remote_publish();
                }
            }
            ClientMsg::Click { col, row } => self.remote_click(conn, col, row),
            ClientMsg::RunTask {
                workspace_id,
                label,
            } => self.remote_run_task(conn, &workspace_id, label),
            ClientMsg::TaskInput { token, value } => self.remote_task_answer(conn, token, value),
            ClientMsg::Hello { .. } | ClientMsg::Ping => {}
        }
    }

    fn with_conn<R>(&self, conn: ConnId, f: impl FnOnce(&mut Conn) -> R) -> Option<R> {
        self.remote
            .rt
            .borrow_mut()
            .as_mut()?
            .conns
            .get_mut(&conn)
            .map(f)
    }

    pub(crate) fn send_to(&self, conn: ConnId, msg: ServerMsg) {
        self.with_conn(conn, |c| c.out.try_send(msg));
    }

    fn attached(&self, conn: ConnId) -> Option<(WorkspaceId, TabId)> {
        self.with_conn(conn, |c| c.attach.as_ref().map(|a| (a.ws, a.tab)))
            .flatten()
    }

    fn attached_term(&self, conn: ConnId) -> Option<Rc<crate::term::TermView>> {
        let key = self.attached(conn)?;
        self.terms.borrow().get(&key).cloned()
    }

    fn remote_attach(self: &Rc<Self>, conn: ConnId, target: Option<(WorkspaceId, TabId)>) {
        self.remote_detach(conn);
        let live = target.filter(|(ws, _)| self.remote_live(*ws).is_ok());
        let view = live.and_then(|key| Some((key, self.terms.borrow().get(&key)?.clone())));
        let Some(((ws, tab), view)) = view else {
            self.send_to(
                conn,
                ServerMsg::Closed {
                    reason: ClosedReason::TabClosed,
                },
            );
            return;
        };
        let colors = Self::remote_defaults();
        self.with_conn(conn, |c| {
            c.attach = Some(Attach {
                ws,
                tab,
                fit: None,
                mirror: Mirror::start(&view.term, c.out.clone(), colors, c.low_bandwidth),
            });
        });
        self.remote_dots();
        self.remote_publish();
    }

    fn remote_detach(self: &Rc<Self>, conn: ConnId) {
        let key = self
            .with_conn(conn, |c| c.attach.take().map(|a| (a.ws, a.tab)))
            .flatten();
        if let Some((ws, tab)) = key {
            self.remote_tab_fit(ws, tab);
            self.remote_publish();
        }
    }

    /// The attached tab closed or its shell exited: release it and tell the phone.
    pub(crate) fn remote_tab_gone(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let gone: Vec<ConnId> = self
            .remote
            .rt
            .borrow()
            .as_ref()
            .map(|rt| {
                rt.conns
                    .iter()
                    .filter(|(_, c)| {
                        c.attach
                            .as_ref()
                            .is_some_and(|a| a.ws == ws && a.tab == tab)
                    })
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default();
        for conn in gone {
            self.remote_detach(conn);
            self.send_to(
                conn,
                ServerMsg::Closed {
                    reason: ClosedReason::TabClosed,
                },
            );
        }
    }

    /// Typed or named-key bytes go to the shell as if the Mac had typed them,
    /// without counting as Mac typing (so the phone fit stays).
    fn remote_write(self: &Rc<Self>, conn: ConnId, bytes: &[u8]) {
        let (Some(view), Some((ws, tab))) = (self.attached_term(conn), self.attached(conn)) else {
            return;
        };
        view.term.feed_child(bytes);
        if self.model.borrow_mut().on_interaction(ws, tab) {
            self.paint_all();
        }
    }

    /// A tap on the live grid, reported to a program that asked for the mouse.
    /// VTE has no getter for its mouse mode, but it shows an arrow pointer
    /// (not the text pointer) exactly while a program reports the mouse. The
    /// report is the SGR form, which every current program asks for.
    fn remote_click(&self, conn: ConnId, col: u16, row: u16) {
        let Some(view) = self.attached_term(conn) else {
            return;
        };
        let term = &view.term;
        let mouse_on = term
            .cursor()
            .and_then(|cursor| cursor.name())
            .is_some_and(|name| name == "default");
        let inside = i64::from(col) < term.column_count() && i64::from(row) < term.row_count();
        if mouse_on && inside {
            let (col, row) = (col + 1, row + 1);
            term.feed_child(format!("\x1b[<0;{col};{row}M\x1b[<0;{col};{row}m").as_bytes());
        }
    }

    fn remote_scroll(&self, conn: ConnId, rows: Option<i32>) {
        let Some(view) = self.attached_term(conn) else {
            return;
        };
        let Some(adj) = view.term.vadjustment() else {
            return;
        };
        let bottom = adj.upper() - adj.page_size();
        let value = match rows {
            // Positive rows show older lines: the viewport moves up.
            Some(rows) => adj.value() - f64::from(rows) * view.term.char_height() as f64,
            None => bottom,
        };
        adj.set_value(value.clamp(adj.lower(), bottom.max(adj.lower())));
    }

    /// Scrollback above the live screen as plain text, oldest first.
    fn remote_history(&self, conn: ConnId) {
        let Some(view) = self.attached_term(conn) else {
            return;
        };
        let Some(adj) = view.term.vadjustment() else {
            return;
        };
        let unit = view.term.char_height().max(1) as f64;
        let live_top = ((adj.upper() - adj.page_size()) / unit).round() as i64;
        let first = ((adj.lower() / unit).round() as i64).max(live_top - HISTORY_LIMIT);
        let lines = if live_top > first {
            let (text, _) = view.term.text_range_format(
                vte::Format::Text,
                first,
                0,
                live_top - 1,
                view.term.column_count(),
            );
            text.map(|t| t.lines().map(|l| l.trim_end().to_string()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.send_to(conn, ServerMsg::History { lines });
    }

    /// Push workspace and attached-tab dots to every phone whose view changed.
    /// Called after every repaint; nothing to do while no phone is connected.
    pub(crate) fn remote_dots(&self) {
        let mut guard = self.remote.rt.borrow_mut();
        let Some(rt) = guard.as_mut().filter(|rt| !rt.conns.is_empty()) else {
            return;
        };
        let model = self.model.borrow();
        let workspaces: BTreeMap<String, Dot> = model
            .registry
            .workspaces
            .iter()
            .filter_map(|w| Some((w.id.to_string(), wire_dot(model.workspace_dot(w.id)?))))
            .collect();
        for conn in rt.conns.values_mut() {
            let attached = conn.attach.as_ref().map(|a| a.ws);
            let tabs = attached
                .map(|ws| {
                    model
                        .terminal_ids(ws)
                        .into_iter()
                        .filter_map(|(tab, _)| Some((tab.0, wire_dot(model.tab_dot(ws, tab)?))))
                        .collect()
                })
                .unwrap_or_default();
            let msg = ServerMsg::Dots {
                workspaces: workspaces.clone(),
                workspace: attached.map(|ws| ws.to_string()),
                tabs,
            };
            if conn.last_dots.as_ref() != Some(&msg) && conn.out.try_send(msg.clone()) {
                conn.last_dots = Some(msg);
            }
        }
    }

    /// The Mac's theme changed: new colors for every phone, rows resent.
    pub(crate) fn remote_theme_changed(&self) {
        let mut guard = self.remote.rt.borrow_mut();
        let Some(rt) = guard.as_mut().filter(|rt| !rt.conns.is_empty()) else {
            return;
        };
        let colors = Self::remote_defaults();
        for conn in rt.conns.values() {
            conn.out.try_send(ServerMsg::Theme {
                theme: xenon_remote::theme_wire(Self::remote_colors(conn.appearance)),
            });
            if let Some(a) = conn.attach.as_ref() {
                a.mirror.set_colors(colors);
            }
        }
    }

    /// Re-dump every terminal a phone watches in this tab (after a resize).
    pub(crate) fn remote_poke_mirrors(&self, ws: WorkspaceId, tab: TabId) {
        if let Some(rt) = self.remote.rt.borrow().as_ref() {
            for a in rt.conns.values().filter_map(|c| c.attach.as_ref()) {
                if a.ws == ws && a.tab == tab {
                    a.mirror.poke();
                }
            }
        }
    }

    /// Sign a device out: tell its phones, drop their sessions.
    pub(crate) fn remote_drop_devices(self: &Rc<Self>, device: Option<&str>) {
        let doomed: Vec<ConnId> = self
            .remote
            .rt
            .borrow()
            .as_ref()
            .map(|rt| {
                rt.conns
                    .iter()
                    .filter(|(_, c)| device.is_none_or(|d| c.device_id == d))
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default();
        for conn in doomed {
            self.send_to(
                conn,
                ServerMsg::Closed {
                    reason: ClosedReason::Revoked,
                },
            );
            self.remote_disconnected(conn);
        }
    }
}
