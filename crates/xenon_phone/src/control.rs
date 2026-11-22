use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::FromRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use crate::api::{PhoneCall, PhoneReply};
use crate::{Phone, locked};

pub(crate) fn serve(phone: &Arc<Phone>, listener: &UnixListener) -> io::Result<()> {
    for stream in listener.incoming() {
        let stream = stream?;
        let phone = Arc::clone(phone);
        thread::spawn(move || {
            let _ = converse(&phone, stream);
        });
    }
    Ok(())
}

pub(crate) fn listener() -> io::Result<UnixListener> {
    match sd_notify::listen_fds()?.next() {
        // SAFETY: systemd passed this listening socket, and nothing else in this process
        // has taken it.
        Some(fd) => Ok(unsafe { UnixListener::from_raw_fd(fd) }),
        None => bind(),
    }
}

fn bind() -> io::Result<UnixListener> {
    let path = xenon_term::phone_socket_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "phone server already running",
            ));
        }
        std::fs::remove_file(&path)?;
    }
    UnixListener::bind(path)
}

fn reply(out: &Mutex<UnixStream>, reply: &PhoneReply) -> io::Result<()> {
    let mut line = serde_json::to_string(reply).map_err(io::Error::other)?;
    line.push('\n');
    locked(out).write_all(line.as_bytes())
}

fn converse(phone: &Arc<Phone>, stream: UnixStream) -> io::Result<()> {
    let out = Mutex::new(stream.try_clone()?);
    let mut watching = None;
    let result = (|| {
        for line in BufReader::new(stream).lines() {
            let answer = match serde_json::from_str::<PhoneCall>(&line?) {
                Ok(PhoneCall::Watch) => {
                    watching.get_or_insert_with(|| watch(phone, &out));
                    PhoneReply::Done
                }
                Ok(call) => phone.call(call),
                Err(e) => PhoneReply::Error {
                    error: e.to_string(),
                },
            };
            reply(&out, &answer)?;
        }
        Ok(())
    })();
    if let Some(id) = watching {
        phone.off_change(id);
    }
    result
}

fn watch(phone: &Arc<Phone>, out: &Mutex<UnixStream>) -> u64 {
    let (tx, rx) = mpsc::channel::<()>();
    let writer = out.lock().ok().and_then(|s| s.try_clone().ok());
    let weak = Arc::downgrade(phone);
    if let Some(stream) = writer {
        thread::spawn(move || {
            let stream = Mutex::new(stream);
            while rx.recv().is_ok() {
                let Some(phone) = weak.upgrade() else { break };
                let status = PhoneReply::Status {
                    status: phone.status(),
                };
                if reply(&stream, &status).is_err() {
                    break;
                }
            }
        });
    }
    let _ = tx.send(());
    let tx = Mutex::new(tx);
    phone.on_change(Box::new(move || {
        let _ = locked(&tx).send(());
    }))
}

impl Phone {
    fn call(&self, call: PhoneCall) -> PhoneReply {
        self.apply_settings();
        match call {
            PhoneCall::Status | PhoneCall::Watch => PhoneReply::Status {
                status: self.status(),
            },
            PhoneCall::Pairing => PhoneReply::Pairing {
                pairing: self.pairing(),
            },
            PhoneCall::Revoke { device } => {
                self.revoke(device.as_deref());
                PhoneReply::Status {
                    status: self.status(),
                }
            }
            PhoneCall::Dark { dark } => {
                self.dark.store(dark, Ordering::Relaxed);
                PhoneReply::Done
            }
        }
    }
}
