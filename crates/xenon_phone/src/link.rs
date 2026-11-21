//! The phone server as the window sees it: pairing code, status rows and the dark flag,
//! over `phone.sock`. The socket is systemd's, so the first call starts the server.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use xenon_remote::RemoteStatus;

use crate::api::{Pairing, PhoneCall, PhoneReply};

const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const RECONNECT: Duration = Duration::from_secs(5);

static DARK: AtomicBool = AtomicBool::new(true);

pub struct PhoneLink;

fn connect() -> Result<UnixStream, String> {
    let path = xenon_term::phone_socket_path();
    let stream = UnixStream::connect(&path).map_err(|e| {
        format!(
            "no phone server at {} ({e}); `scripts/release/install` sets up its units",
            path.display()
        )
    })?;
    stream
        .set_read_timeout(Some(CALL_TIMEOUT))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(CALL_TIMEOUT))
        .map_err(|e| e.to_string())?;
    Ok(stream)
}

fn send(stream: &mut UnixStream, call: &PhoneCall) -> Result<(), String> {
    let mut line = serde_json::to_string(call).map_err(|e| e.to_string())?;
    line.push('\n');
    stream.write_all(line.as_bytes()).map_err(|e| e.to_string())
}

fn ask(call: &PhoneCall) -> Result<PhoneReply, String> {
    let mut stream = connect()?;
    send(&mut stream, call)?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    match serde_json::from_str(&line).map_err(|e| e.to_string())? {
        PhoneReply::Error { error } => Err(error),
        reply => Ok(reply),
    }
}

fn status_of(call: &PhoneCall) -> Result<RemoteStatus, String> {
    match ask(call)? {
        PhoneReply::Status { status } => Ok(status),
        _ => Err("unexpected reply".into()),
    }
}

impl PhoneLink {
    pub fn status() -> Result<RemoteStatus, String> {
        status_of(&PhoneCall::Status)
    }

    /// Sign out one device, or all of them.
    pub fn revoke(device: Option<&str>) -> Result<RemoteStatus, String> {
        status_of(&PhoneCall::Revoke {
            device: device.map(str::to_owned),
        })
    }

    /// The current pairing code. Turn the phone remote on in `services.json` first.
    pub fn pairing() -> Result<Pairing, String> {
        match ask(&PhoneCall::Pairing)? {
            PhoneReply::Pairing { pairing } => pairing,
            _ => Err("unexpected reply".into()),
        }
    }

    /// Whether the window's theme is dark. Remembered and sent again whenever the
    /// connection comes back, so a restarted phone server learns it.
    pub fn set_dark(dark: bool) {
        DARK.store(dark, Ordering::Relaxed);
        thread::spawn(move || {
            let _ = ask(&PhoneCall::Dark { dark });
        });
    }

    /// A message each time the phone server's status changes (a phone connects, a
    /// device is paired). Reconnects by itself, so it survives a server restart.
    pub fn watch() -> async_channel::Receiver<()> {
        let (tx, rx) = async_channel::unbounded();
        thread::spawn(move || {
            while !tx.is_closed() {
                let _ = follow(&tx);
                thread::sleep(RECONNECT);
            }
        });
        rx
    }
}

fn follow(tx: &async_channel::Sender<()>) -> Result<(), String> {
    let mut stream = connect()?;
    send(
        &mut stream,
        &PhoneCall::Dark {
            dark: DARK.load(Ordering::Relaxed),
        },
    )?;
    send(&mut stream, &PhoneCall::Watch)?;
    // Pushes can be far apart: only the calls above have a deadline.
    stream.set_read_timeout(None).map_err(|e| e.to_string())?;
    // Connected (again): the window refreshes what it shows.
    tx.send_blocking(()).map_err(|e| e.to_string())?;
    for line in BufReader::new(stream).lines() {
        if let Ok(PhoneReply::Changed) = serde_json::from_str(&line.map_err(|e| e.to_string())?) {
            tx.send_blocking(()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
