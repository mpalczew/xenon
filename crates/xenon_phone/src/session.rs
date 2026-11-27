use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use crate::remote::{
    AppearanceWire, ClientMsg, ClosedReason, ConnId, NamedKey, Outbox, ServerMsg,
    device_short_name, parse_workspace_id, pty_text_bytes,
};
use xenon_core::{TermId, WorkspaceId};
use xenon_term::{Mirror, Modes, Size};

use crate::look::Look;
use crate::window::Report;

use super::{Phone, frames, locked};

const WHEEL_LIMIT: i32 = 40;

pub(crate) struct View {
    pub low_bandwidth: AtomicBool,
    pub look: AtomicU8,
    pub full: AtomicBool,
    report: Mutex<Option<Report>>,
}

impl View {
    pub fn report(&self) -> Option<Report> {
        *locked(&self.report)
    }
}

/// The view and mirror of a live attach.
type Attached = (Arc<View>, Arc<Mirror>);

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

impl Conn {
    fn attached(&self) -> Option<Attached> {
        self.attach
            .as_ref()
            .map(|a| (Arc::clone(&a.view), Arc::clone(&a.mirror)))
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

    pub(crate) fn connected(&self, conn: ConnId, device_id: String, out: Outbox) {
        let host_name = locked(&self.server)
            .rt
            .as_ref()
            .map(|rt| rt.host_name.clone())
            .unwrap_or_default();
        let theme = Look::of(self.dark(0)).theme();
        let build = xenon_term::BUILD.to_owned();
        out.try_send(ServerMsg::Ready {
            theme,
            host_name,
            build,
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

    pub(crate) fn disconnected(&self, conn: ConnId) {
        self.detach(conn);
        self.task_gone(conn);
        locked(&self.conns).remove(&conn);
        self.changed();
    }

    pub(crate) fn message(&self, conn: ConnId, msg: ClientMsg) {
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
        let typed = matches!(msg, ClientMsg::Text { .. } | ClientMsg::Key { .. });
        let Some((view, mirror)) = locked(&self.conns).get(&conn).and_then(Conn::attached) else {
            if typed {
                eprintln!("xenon-phone input dropped conn={}: not attached", conn.0);
            }
            return;
        };
        if typed && mirror.is_gone() {
            eprintln!(
                "xenon-phone input dropped conn={}: terminal connection gone",
                conn.0
            );
        }
        match msg {
            ClientMsg::Text { data } => mirror.input(&pty_text_bytes(&data)),
            ClientMsg::Key { key } => mirror.input(key.pty_bytes(mirror.modes().app_cursor)),
            ClientMsg::Wheel { rows, col, row } => {
                alt_wheel(&mirror, rows.clamp(-WHEEL_LIMIT, WHEEL_LIMIT), col, row);
            }
            ClientMsg::Scroll {
                epoch,
                top,
                follow,
                lines,
            } => scrolled(
                &view,
                &mirror,
                Report {
                    epoch,
                    top,
                    follow,
                    lines,
                },
            ),
            ClientMsg::Click { col, row } => click(&mirror, col, row),
            ClientMsg::Fit { cols, rows, diag } => {
                if let Some(size) = Size::new(cols, rows) {
                    let detail: String = diag
                        .as_deref()
                        .unwrap_or("")
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(200)
                        .collect();
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
            eprintln!("xenon-phone attach conn={} failed", conn.0);
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
                report: Mutex::new(None),
            });
            frames::start(self, &mirror, &view, c.out.clone());
            c.attach = Some(Attach {
                term: info.id,
                workspace: info.workspace,
                view,
                mirror: Arc::clone(&mirror),
            });
        }
        eprintln!("xenon-phone attach conn={} term={}", conn.0, info.id);
        mirror.wake();
        self.push_dots();
        self.changed();
    }

    pub(super) fn closed_notice(&self, conn: ConnId) {
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
            eprintln!("xenon-phone detach conn={} term={}", conn.0, a.term);
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
            c.attached()
        };
        let code = look_code(appearance);
        let theme = Look::of(self.dark(code)).theme();
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

/// Where the phone is scrolled. The next frame tick plans from it.
fn scrolled(view: &View, mirror: &Mirror, report: Report) {
    *locked(&view.report) = Some(report);
    mirror.wake();
}

fn click(mirror: &Mirror, col: u16, row: u16) {
    let modes = mirror.modes();
    if !modes.mouse {
        return;
    }
    let press = mouse_report(&modes, 0, col, row, true);
    let release = mouse_report(&modes, 3, col, row, false);
    if let (Some(press), Some(release)) = (press, release) {
        mirror.input(&[press, release].concat());
    }
}

/// Wheel in the alt screen: wheel reports when the app asked for the mouse, else arrow
/// keys when it asked for alternate scroll. `rows` above zero scrolls up.
fn alt_wheel(mirror: &Mirror, rows: i32, col: u16, row: u16) {
    let modes = mirror.modes();
    let up = rows > 0;
    let step = if modes.mouse {
        mouse_report(&modes, if up { 64 } else { 65 }, col, row, true)
    } else if modes.alt && modes.alt_scroll {
        let key = if up { NamedKey::Up } else { NamedKey::Down };
        Some(key.pty_bytes(modes.app_cursor).to_vec())
    } else {
        None
    };
    if let Some(step) = step {
        mirror.input(&step.repeat(rows.unsigned_abs() as usize));
    }
}

/// One mouse report at a cell; `button` 3 is a release in X10, which SGR spells as `m`.
fn mouse_report(modes: &Modes, button: u8, col: u16, row: u16, press: bool) -> Option<Vec<u8>> {
    let size = modes.size;
    if col >= size.cols() || row >= size.rows() {
        return None;
    }
    let (c, r) = (u32::from(col) + 1, u32::from(row) + 1);
    if modes.sgr_mouse {
        let (b, end) = if press { (button, 'M') } else { (0, 'm') };
        return Some(format!("\x1b[<{b};{c};{r}{end}").into_bytes());
    }
    (c <= 222 && r <= 222).then(|| {
        vec![
            0x1b,
            b'[',
            b'M',
            32 + button,
            (32 + c) as u8,
            (32 + r) as u8,
        ]
    })
}
