use std::io::Write;
use std::net::{Shutdown, TcpStream};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::{CloseFrame, Role, WebSocketConfig};
use tungstenite::{Message, WebSocket};

use crate::host::{ConnId, HostRequest, Outbox};
use crate::http::{self, Request};
use crate::protocol::{ClientMsg, ClosedReason, ServerMsg};
use crate::server::Shared;

const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const PING_EVERY: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(1);
const UNAUTHORIZED: u16 = 4401;
const TRY_AGAIN: u16 = 1011;
const MAX_MESSAGE: usize = 64 * 1024;

type Socket = WebSocket<TcpStream>;

pub(crate) fn serve(mut stream: TcpStream, req: &Request, shared: &Shared) -> Result<()> {
    if !same_origin(req) {
        return http::write_json_error(&mut stream, 403, "cross-origin");
    }
    let key = req
        .header("sec-websocket-key")
        .ok_or_else(|| anyhow!("missing websocket key"))?;
    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
    write!(
        stream,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    )?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(HELLO_TIMEOUT))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let config = WebSocketConfig {
        max_message_size: Some(MAX_MESSAGE),
        max_frame_size: Some(MAX_MESSAGE),
        max_write_buffer_size: 1024 * 1024,
        ..Default::default()
    };
    // One socket per direction over the same TCP stream: the reader blocks in
    // `read` while the writer sends, so they cannot share one WebSocket.
    let mut reader = WebSocket::from_raw_socket(stream.try_clone()?, Role::Server, Some(config));
    let writer = Mutex::new(WebSocket::from_raw_socket(
        stream,
        Role::Server,
        Some(config),
    ));
    let device_id = match await_hello(&mut reader, shared) {
        Hello::Device(id) => id,
        Hello::Unknown => {
            close(&mut lock(&writer), UNAUTHORIZED, "unauthorized");
            return Ok(());
        }
        Hello::HostBusy => {
            close(&mut lock(&writer), TRY_AGAIN, "busy");
            return Ok(());
        }
    };
    reader.get_ref().set_read_timeout(None)?;
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
    let reading = AtomicBool::new(true);
    let result = thread::scope(|scope| {
        let (writer, reading) = (&writer, &reading);
        scope.spawn(move || write_loop(writer, &rx, reading, shared));
        let result = read_loop(&mut reader, writer, conn, shared);
        reading.store(false, Ordering::SeqCst);
        result
    });
    let _ = shared
        .host
        .send_blocking(HostRequest::Disconnected { conn });
    result
}

/// Browsers always send Origin on WebSocket; it must be this server.
fn same_origin(req: &Request) -> bool {
    let Some(origin) = req.header("origin") else {
        return true;
    };
    origin == format!("http://{}", req.header("host").unwrap_or(""))
}

enum Hello {
    Device(String),
    Unknown,
    HostBusy,
}

fn await_hello(socket: &mut Socket, shared: &Shared) -> Hello {
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
            Read::Other => {}
        }
    }
    Hello::Unknown
}

fn read_loop(
    socket: &mut Socket,
    writer: &Mutex<Socket>,
    conn: ConnId,
    shared: &Shared,
) -> Result<()> {
    loop {
        match read(socket) {
            Read::Msg(ClientMsg::Ping) => send(&mut lock(writer), &ServerMsg::Pong)?,
            Read::Msg(msg) => shared
                .host
                .send_blocking(HostRequest::Message { conn, msg })
                .map_err(|_| anyhow!("host gone"))?,
            Read::Other => {}
            Read::Closed => return Ok(()),
        }
    }
}

/// Sends queued messages and pings until the session ends, then closes the
/// stream, which also wakes the reader.
fn write_loop(
    writer: &Mutex<Socket>,
    rx: &Receiver<ServerMsg>,
    reading: &AtomicBool,
    shared: &Shared,
) {
    if let Ok(Some((code, reason))) = pump(writer, rx, reading, shared) {
        close(&mut lock(writer), code, reason);
    }
    let _ = lock(writer).get_ref().shutdown(Shutdown::Both);
}

/// Returns the close frame to send, if the stream can still take one.
fn pump(
    writer: &Mutex<Socket>,
    rx: &Receiver<ServerMsg>,
    reading: &AtomicBool,
    shared: &Shared,
) -> Result<Option<(u16, &'static str)>> {
    let mut last_ping = Instant::now();
    loop {
        if !reading.load(Ordering::SeqCst) {
            return Ok(Some((1000, "")));
        }
        if !shared.running() {
            let stopping = ServerMsg::Closed {
                reason: ClosedReason::ServerStopping,
            };
            send(&mut lock(writer), &stopping)?;
            return Ok(Some((1001, "server stopping")));
        }
        match rx.recv_timeout(CHECK_EVERY) {
            Ok(msg) => {
                let revoked = matches!(
                    msg,
                    ServerMsg::Closed {
                        reason: ClosedReason::Revoked
                    }
                );
                send(&mut lock(writer), &msg)?;
                if revoked {
                    return Ok(Some((UNAUTHORIZED, "revoked")));
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(Some((TRY_AGAIN, "session ended"))),
        }
        if last_ping.elapsed() >= PING_EVERY {
            lock(writer).send(Message::Ping(Vec::new()))?;
            last_ping = Instant::now();
        }
    }
}

fn lock(writer: &Mutex<Socket>) -> std::sync::MutexGuard<'_, Socket> {
    writer.lock().unwrap_or_else(|e| e.into_inner())
}

fn send(socket: &mut Socket, msg: &ServerMsg) -> Result<()> {
    socket.send(Message::Text(serde_json::to_string(msg)?))?;
    Ok(())
}

enum Read {
    Msg(ClientMsg),
    Other,
    Closed,
}

fn read(socket: &mut Socket) -> Read {
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
            Read::Other
        }
        Err(_) => Read::Closed,
    }
}

fn close(socket: &mut Socket, code: u16, reason: &str) {
    let _ = socket.close(Some(CloseFrame {
        code: CloseCode::from(code),
        reason: reason.to_string().into(),
    }));
    let _ = socket.flush();
}
