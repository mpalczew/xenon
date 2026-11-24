use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::remote::{
    AppearanceWire, ClientMsg, ClosedReason, ConnId, Outbox, ServerMsg, device_short_name,
    parse_workspace_id, pty_text_bytes,
};
use xenon_core::{TermId, WorkspaceId};
use xenon_term::{Mirror, Size};

use crate::look::Look;

use super::{Phone, frames, locked};

const HISTORY_LIMIT: usize = 1000;
const WHEEL_LIMIT: i32 = 40;

pub(crate) struct View {
    pub low_bandwidth: AtomicBool,
    pub look: AtomicU8,
    pub full: AtomicBool,
}

pub(crate) struct Attach {
    pub term: TermId,
    pub workspace: WorkspaceId,
    pub view: Arc<View>,
    pub mirror: Arc<Mirror>,
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
            _ => self.dark.load(Ordering::Relaxed),
        }
    }

    pub(super) fn connected(&self, conn: ConnId, device_id: String, out: Outbox) {
        let host_name = locked(&self.server)
            .rt
            .as_ref()
            .map(|rt| rt.host_name.clone())
            .unwrap_or_default();
        let theme = crate::remote::theme_wire(Look::of(self.dark(0)).theme());
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

    fn attached(&self, conn: ConnId) -> Option<Arc<Mirror>> {
        let guard = locked(&self.conns);
        let a = guard.get(&conn)?.attach.as_ref()?;
        Some(Arc::clone(&a.mirror))
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

    fn terminal_message(&self, conn: ConnId, msg: ClientMsg) {
        let Some(mirror) = self.attached(conn) else {
            return;
        };
        match msg {
            ClientMsg::Text { data } => mirror.input(&pty_text_bytes(&data)),
            ClientMsg::Key { key } => mirror.input(key.pty_bytes(mirror.modes().app_cursor)),
            ClientMsg::History => {
                let lines =
                    mirror.read(|term, _| crate::screen::history_lines(term, HISTORY_LIMIT));
                self.send_to(conn, ServerMsg::History { lines });
            }
            ClientMsg::Wheel { rows } => mirror.scroll(rows.clamp(-WHEEL_LIMIT, WHEEL_LIMIT)),
            ClientMsg::Bottom => mirror.bottom(),
            ClientMsg::Click { col, row } => click(&mirror, col, row),
            ClientMsg::Fit { cols, rows, diag } => {
                if let Some(size) = Size::new(cols, rows) {
                    let detail = diag.as_deref().unwrap_or("");
                    eprintln!("xenon-phone fit conn={} {cols}x{rows} {detail}", conn.0);
                    mirror.fit(size);
                    self.changed();
                }
            }
            _ => {}
        }
    }

    fn attach(&self, conn: ConnId, workspace_id: &str, tab_id: u64) {
        self.detach(conn);
        let found = parse_workspace_id(workspace_id)
            .ok()
            .and_then(|ws| self.find(ws, tab_id));
        let Some(info) = found else {
            self.closed_notice(conn);
            return;
        };
        let label = self.device_label(conn);
        let Ok(mirror) = self.terms.watch(info.id, &label) else {
            self.closed_notice(conn);
            return;
        };
        let mirror = Arc::new(mirror);
        {
            let mut guard = locked(&self.conns);
            let Some(c) = guard.get_mut(&conn) else {
                return;
            };
            let view = Arc::new(View {
                low_bandwidth: AtomicBool::new(c.low_bandwidth),
                look: AtomicU8::new(look_code(c.appearance)),
                full: AtomicBool::new(false),
            });
            frames::start(self, &mirror, &view, c.out.clone());
            c.attach = Some(Attach {
                term: info.id,
                workspace: info.workspace,
                view,
                mirror: Arc::clone(&mirror),
            });
        }
        mirror.wake();
        self.push_dots();
        self.changed();
    }

    fn closed_notice(&self, conn: ConnId) {
        self.send_to(
            conn,
            ServerMsg::Closed {
                reason: ClosedReason::TabClosed,
            },
        );
    }

    fn detach_and_report(&self, conn: ConnId) {
        self.detach(conn);
        self.push_dots();
        self.changed();
    }

    pub(super) fn detach(&self, conn: ConnId) {
        let attach = locked(&self.conns)
            .get_mut(&conn)
            .and_then(|c| c.attach.take());
        if let Some(a) = attach {
            a.mirror.stop();
        }
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
                .map(|a| (Arc::clone(&a.view), Arc::clone(&a.mirror)))
        };
        let code = look_code(appearance);
        let theme = crate::remote::theme_wire(Look::of(self.dark(code)).theme());
        self.send_to(conn, ServerMsg::Theme { theme });
        if let Some((view, mirror)) = attach {
            view.look.store(code, Ordering::Relaxed);
            view.full.store(true, Ordering::Relaxed);
            mirror.wake();
        }
    }

    fn device_label(&self, conn: ConnId) -> String {
        let device = locked(&self.conns)
            .get(&conn)
            .map(|c| c.device_id.clone())
            .unwrap_or_default();
        locked(&self.server)
            .rt
            .as_ref()
            .and_then(|rt| {
                rt.devices
                    .label(&device)
                    .map(device_short_name)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "Phone".into())
    }
}

fn click(mirror: &Mirror, col: u16, row: u16) {
    let modes = mirror.modes();
    let size = modes.size;
    if !modes.mouse || col >= size.cols() || row >= size.rows() {
        return;
    }
    let (c, r) = (u32::from(col) + 1, u32::from(row) + 1);
    let bytes = if modes.sgr_mouse {
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
    mirror.input(&bytes);
}
