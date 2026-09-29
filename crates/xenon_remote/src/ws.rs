//! `/ws`: one WebSocket per phone app session. Hello → Connected → a loop that
//! forwards client messages to the host and drains the host's outbox.

use std::net::TcpStream;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::{CloseFrame, Role, WebSocketConfig};
use tungstenite::{Message, WebSocket};

use crate::host::{ConnId, HostRequest, Outbox};
use crate::http;
use crate::protocol::{ClientMsg, ClosedReason, ServerMsg};
use crate::server::{Request, Shared};

/// Poll granularity: how long a read may block before the outbox is drained.
const TICK: Duration = Duration::from_millis(20);
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const PING_EVERY: Duration = Duration::from_secs(20);
/// Close code for a missing / revoked device token (the phone forgets its token).
const UNAUTHORIZED: u16 = 4401;
/// Close code for "try again": host busy, or this session ended on the Mac.
const TRY_AGAIN: u16 = 1011;
/// Client messages are tiny (keys, text lines); cap before auth.
const MAX_MESSAGE: usize = 64 * 1024;

pub(crate) fn serve(mut stream: TcpStream, req: &Request, shared: &Shared) -> Result<()> {
    if !same_origin(req) {
        return http::write_json_error(&mut stream, 403, "cross-origin");
    }
    let key = req
        .headers
        .get("sec-websocket-key")
        .ok_or_else(|| anyhow!("missing websocket key"))?;
    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
    http::write_ws_accept(&mut stream, &accept)?;
    stream.set_read_timeout(Some(TICK))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let config = WebSocketConfig {
        max_message_size: Some(MAX_MESSAGE),
        max_frame_size: Some(MAX_MESSAGE),
        ..Default::default()
    };
    let mut socket = WebSocket::from_raw_socket(stream, Role::Server, Some(config));
    let device_id = match await_hello(&mut socket, shared) {
        Hello::Device(id) => id,
        Hello::Unknown => {
            close(&mut socket, UNAUTHORIZED, "unauthorized");
            return Ok(());
        }
        Hello::HostBusy => {
            close(&mut socket, TRY_AGAIN, "busy");
            return Ok(());
        }
    };
    let conn = shared.next_conn();
    let (out, rx) = Outbox::channel();
    shared
        .host
        .send_blocking(HostRequest::Connected {
            conn,
            device_id,
            out,
        })
        .map_err(|_| anyhow!("host gone"))?;
    let result = session(&mut socket, conn, &rx, shared);
    let _ = shared
        .host
        .send_blocking(HostRequest::Disconnected { conn });
    result
}

/// Browsers always send Origin on WebSocket; it must be this server.
fn same_origin(req: &Request) -> bool {
    let Some(origin) = req.headers.get("origin") else {
        return true; // Non-browser client (tests, curl).
    };
    let host = req.headers.get("host").map(String::as_str).unwrap_or("");
    origin == &format!("http://{host}")
}

enum Hello {
    Device(String),
    /// Bad, revoked, or missing token.
    Unknown,
    /// The Mac didn't answer; the token may be fine.
    HostBusy,
}

fn await_hello(socket: &mut WebSocket<TcpStream>, shared: &Shared) -> Hello {
    let deadline = Instant::now() + HELLO_TIMEOUT;
    while Instant::now() < deadline {
        match read(socket) {
            Read::Msg(ClientMsg::Hello { token }) => {
                return match shared.authenticate(&token) {
                    Ok(Some(id)) => Hello::Device(id),
                    Ok(None) => Hello::Unknown,
                    Err(_) => Hello::HostBusy,
                };
            }
            Read::Msg(_) | Read::Closed => return Hello::Unknown,
            Read::Idle | Read::Other => {}
        }
    }
    Hello::Unknown
}

fn session(
    socket: &mut WebSocket<TcpStream>,
    conn: ConnId,
    rx: &Receiver<ServerMsg>,
    shared: &Shared,
) -> Result<()> {
    let mut last_ping = Instant::now();
    loop {
        if !shared.running() {
            send(
                socket,
                &ServerMsg::Closed {
                    reason: ClosedReason::ServerStopping,
                },
            )?;
            close(socket, 1001, "server stopping");
            return Ok(());
        }
        match drain(socket, rx)? {
            Drain::Open => {}
            Drain::Revoked => {
                close(socket, UNAUTHORIZED, "revoked");
                return Ok(());
            }
            Drain::Ended => {
                close(socket, TRY_AGAIN, "session ended");
                return Ok(());
            }
        }
        match read(socket) {
            Read::Msg(ClientMsg::Ping) => send(socket, &ServerMsg::Pong)?,
            Read::Msg(msg) => shared
                .host
                .send_blocking(HostRequest::Message { conn, msg })
                .map_err(|_| anyhow!("host gone"))?,
            Read::Closed => return Ok(()),
            Read::Idle | Read::Other => {}
        }
        if last_ping.elapsed() >= PING_EVERY {
            socket.send(Message::Ping(Vec::new()))?;
            last_ping = Instant::now();
        }
    }
}

enum Drain {
    Open,
    /// The Mac revoked this device: the phone must forget its token.
    Revoked,
    /// The Mac dropped the session (stopping, restarting): reconnect later.
    Ended,
}

/// Send everything queued.
fn drain(socket: &mut WebSocket<TcpStream>, rx: &Receiver<ServerMsg>) -> Result<Drain> {
    loop {
        match rx.try_recv() {
            Ok(msg) => {
                let revoked = matches!(
                    msg,
                    ServerMsg::Closed {
                        reason: ClosedReason::Revoked
                    }
                );
                send(socket, &msg)?;
                if revoked {
                    return Ok(Drain::Revoked);
                }
            }
            Err(TryRecvError::Empty) => return Ok(Drain::Open),
            Err(TryRecvError::Disconnected) => return Ok(Drain::Ended),
        }
    }
}

fn send(socket: &mut WebSocket<TcpStream>, msg: &ServerMsg) -> Result<()> {
    let json = serde_json::to_string(msg)?;
    socket.send(Message::Text(json))?;
    Ok(())
}

enum Read {
    Msg(ClientMsg),
    /// No complete message within one tick.
    Idle,
    /// Ping/pong/binary/unparseable: ignored.
    Other,
    Closed,
}

fn read(socket: &mut WebSocket<TcpStream>) -> Read {
    match socket.read() {
        Ok(Message::Text(text)) => match serde_json::from_str::<ClientMsg>(&text) {
            Ok(msg) => Read::Msg(msg),
            Err(e) => {
                log::debug!("xenon remote: bad client message: {e}");
                Read::Other
            }
        },
        Ok(Message::Close(_)) => Read::Closed,
        Ok(_) => Read::Other,
        Err(tungstenite::Error::Io(e))
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            // Flush any queued pong the read produced.
            let _ = socket.flush();
            Read::Idle
        }
        Err(_) => Read::Closed,
    }
}

fn close(socket: &mut WebSocket<TcpStream>, code: u16, reason: &str) {
    let _ = socket.close(Some(CloseFrame {
        code: CloseCode::from(code),
        reason: reason.to_string().into(),
    }));
    let _ = socket.flush();
}
