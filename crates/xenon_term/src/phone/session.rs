//! One phone's WebSocket session: what it is attached to, its own scroll and look, and
//! the messages it sends. A phone never moves the computer: scroll, size and look are
//! per phone.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

use alacritty_terminal::term::TermMode;
use xenon_core::WorkspaceId;
use xenon_remote::{
    AppearanceWire, ClientMsg, ClosedReason, ConnId, Outbox, ServerMsg, clamp_fit,
    device_short_name, parse_workspace_id, pty_text_bytes,
};

use super::colors::Look;
use super::{PHONE_BIT, Phone, Watch, frames, locked};
use crate::emu_view;
use crate::term::Term;
use crate::{Claim, Size};

const HISTORY_LIMIT: usize = 1000;
const WHEEL_LIMIT: i32 = 40;

/// What a phone sees of its terminal, shared with the thread that draws frames.
pub(crate) struct View {
    pub offset: AtomicUsize,
    pub low_bandwidth: AtomicBool,
    /// 0 follows the computer's theme, 1 dark, 2 light.
    pub look: AtomicU8,
    pub full: AtomicBool,
}

pub(crate) struct Attach {
    pub term: Arc<Term>,
    pub view: Arc<View>,
    pub watch: Arc<Watch>,
}

pub(crate) struct Conn {
    pub device_id: String,
    pub out: Outbox,
    pub attach: Option<Attach>,
    pub appearance: Option<AppearanceWire>,
    pub low_bandwidth: bool,
    pub last_dots: Option<ServerMsg>,
}

fn look_code(appearance: Option<AppearanceWire>) -> u8 {
    match appearance {
        None => 0,
        Some(AppearanceWire::Dark) => 1,
        Some(AppearanceWire::Light) => 2,
    }
}

impl Phone {
    pub(crate) fn dark(&self, code: u8) -> bool {
        match code {
            1 => true,
            2 => false,
            _ => self.terms.dark.load(Ordering::Relaxed),
        }
    }

    pub(super) fn connected(&self, conn: ConnId, device_id: String, out: Outbox) {
        let host_name = locked(&self.server)
            .rt
            .as_ref()
            .map(|rt| rt.host_name.clone())
            .unwrap_or_default();
        let theme = xenon_remote::theme_wire(Look::of(self.dark(0)).theme());
        out.try_send(ServerMsg::Ready {
            theme,
            device_id: device_id.clone(),
            host_name,
        });
        locked(&self.conns).insert(
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
        self.push_dots();
        self.changed();
    }

    pub(super) fn disconnected(&self, conn: ConnId) {
        self.detach(conn);
        self.task_gone(conn);
        locked(&self.conns).remove(&conn);
        self.changed();
    }

    fn find(&self, ws: WorkspaceId, serial: u64) -> Option<Arc<Term>> {
        self.terms
            .all()
            .into_iter()
            .find(|t| t.workspace == ws && t.serial == serial)
    }

    /// The conn's terminal, its view and its watch.
    fn attached(&self, conn: ConnId) -> Option<(Arc<Term>, Arc<View>, Arc<Watch>)> {
        let guard = locked(&self.conns);
        let a = guard.get(&conn)?.attach.as_ref()?;
        Some((
            Arc::clone(&a.term),
            Arc::clone(&a.view),
            Arc::clone(&a.watch),
        ))
    }

    pub(super) fn message(&self, conn: ConnId, msg: ClientMsg) {
        match msg {
            ClientMsg::Attach {
                workspace_id,
                tab_id,
            } => self.attach(conn, &workspace_id, tab_id),
            ClientMsg::Detach => self.detach_and_report(conn),
            ClientMsg::Bandwidth { low_bandwidth } => self.bandwidth(conn, low_bandwidth),
            ClientMsg::Appearance { appearance } => self.appearance(conn, appearance),
            ClientMsg::RunTask {
                workspace_id,
                label,
            } => self.run_task(conn, &workspace_id, label),
            ClientMsg::TaskInput { token, value } => self.task_answer(conn, token, value),
            ClientMsg::Hello { .. } | ClientMsg::Ping => {}
            other => self.terminal_message(conn, other),
        }
    }

    /// Messages that act on the attached terminal.
    fn terminal_message(&self, conn: ConnId, msg: ClientMsg) {
        let Some((term, view, watch)) = self.attached(conn) else {
            return;
        };
        match msg {
            ClientMsg::Text { data } => self.terms.input(term.id, &pty_text_bytes(&data), false),
            ClientMsg::Key { key } => {
                let app_cursor = term.lock().emu.mode().contains(TermMode::APP_CURSOR);
                self.terms.input(term.id, key.pty_bytes(app_cursor), false);
            }
            ClientMsg::History => {
                let lines = emu_view::history_lines(&term.lock().emu, HISTORY_LIMIT);
                self.send_to(conn, ServerMsg::History { lines });
            }
            ClientMsg::Wheel { rows } => {
                let rows = rows.clamp(-WHEEL_LIMIT, WHEEL_LIMIT);
                let max = emu_view::history_size(&term.lock().emu) as i64;
                let now = view.offset.load(Ordering::Relaxed) as i64;
                view.offset.store(
                    (now + i64::from(rows)).clamp(0, max) as usize,
                    Ordering::Relaxed,
                );
                watch.poke();
            }
            ClientMsg::Bottom => {
                view.offset.store(0, Ordering::Relaxed);
                watch.poke();
            }
            ClientMsg::Click { col, row } => click(&self.terms, &term, col, row),
            ClientMsg::Fit { cols, rows } => self.fit(conn, &term, cols, rows),
            _ => {}
        }
    }

    fn attach(&self, conn: ConnId, workspace_id: &str, tab_id: u64) {
        self.detach(conn);
        let term = parse_workspace_id(workspace_id)
            .ok()
            .and_then(|ws| self.find(ws, tab_id));
        let Some(term) = term else {
            self.send_to(
                conn,
                ServerMsg::Closed {
                    reason: ClosedReason::TabClosed,
                },
            );
            return;
        };
        let attach = {
            let mut guard = locked(&self.conns);
            let Some(c) = guard.get_mut(&conn) else {
                return;
            };
            let view = Arc::new(View {
                offset: AtomicUsize::new(0),
                low_bandwidth: AtomicBool::new(c.low_bandwidth),
                look: AtomicU8::new(look_code(c.appearance)),
                full: AtomicBool::new(false),
            });
            let watch = Arc::new(Watch::default());
            frames::start(self, &term, &watch, &view, c.out.clone());
            c.attach = Some(Attach {
                term: Arc::clone(&term),
                view,
                watch: Arc::clone(&watch),
            });
            watch
        };
        term.lock().watchers.push(Arc::clone(&attach));
        attach.poke();
        self.push_dots();
        self.changed();
    }

    fn detach_and_report(&self, conn: ConnId) {
        self.detach(conn);
        self.push_dots();
        self.changed();
    }

    /// Stop watching and give back the size this phone claimed.
    pub(super) fn detach(&self, conn: ConnId) {
        let attach = locked(&self.conns)
            .get_mut(&conn)
            .and_then(|c| c.attach.take());
        let Some(a) = attach else { return };
        a.watch.stop();
        a.term.lock().watchers.retain(|w| !Arc::ptr_eq(w, &a.watch));
        self.terms.hub.emit(&a.term.detach(conn.0 | PHONE_BIT));
    }

    fn bandwidth(&self, conn: ConnId, low: bool) {
        if let Some(c) = locked(&self.conns).get_mut(&conn) {
            c.low_bandwidth = low;
            if let Some(a) = &c.attach {
                a.view.low_bandwidth.store(low, Ordering::Relaxed);
            }
        }
    }

    fn appearance(&self, conn: ConnId, appearance: Option<AppearanceWire>) {
        let attach = {
            let mut guard = locked(&self.conns);
            let Some(c) = guard.get_mut(&conn) else {
                return;
            };
            c.appearance = appearance;
            c.attach
                .as_ref()
                .map(|a| (Arc::clone(&a.view), Arc::clone(&a.watch)))
        };
        let code = look_code(appearance);
        let theme = xenon_remote::theme_wire(Look::of(self.dark(code)).theme());
        self.send_to(conn, ServerMsg::Theme { theme });
        if let Some((view, watch)) = attach {
            view.look.store(code, Ordering::Relaxed);
            view.full.store(true, Ordering::Relaxed);
            watch.poke();
        }
    }

    fn fit(&self, conn: ConnId, term: &Term, cols: u16, rows: u16) {
        let (cols, rows) = clamp_fit(cols, rows);
        let Some(size) = Size::new(cols, rows) else {
            return;
        };
        let device = locked(&self.conns)
            .get(&conn)
            .map(|c| c.device_id.clone())
            .unwrap_or_default();
        let label = locked(&self.server)
            .rt
            .as_ref()
            .and_then(|rt| {
                rt.devices
                    .label(&device)
                    .map(device_short_name)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "Phone".into());
        self.terms
            .claim(term.id, conn.0 | PHONE_BIT, Claim::Phone { size, label });
        self.changed();
    }
}

/// A tap on the live grid, reported to a program that asked for the mouse: SGR when it
/// asked for that, the classic encoding otherwise.
fn click(terms: &crate::terms::Terms, term: &Term, col: u16, row: u16) {
    let (mode, size) = {
        let st = term.lock();
        (st.emu.mode(), st.emu.size)
    };
    let wants = mode
        .intersects(TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION);
    if !wants || col >= size.cols() || row >= size.rows() {
        return;
    }
    let (c, r) = (u32::from(col) + 1, u32::from(row) + 1);
    let bytes = if mode.contains(TermMode::SGR_MOUSE) {
        format!("\x1b[<0;{c};{r}M\x1b[<0;{c};{r}m").into_bytes()
    } else if c <= 222 && r <= 222 {
        vec![
            0x1b,
            b'[',
            b'M',
            32,
            (32 + c) as u8,
            (32 + r) as u8,
            0x1b,
            b'[',
            b'M',
            35,
            (32 + c) as u8,
            (32 + r) as u8,
        ]
    } else {
        return;
    };
    terms.input(term.id, &bytes, false);
}
