//! Integration tests: real sockets against a mock host.

use super::*;
use crate::host::Outbox;
use crate::protocol::{ClientMsg, Dot, ServerMsg, ThemeWire};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tungstenite::Message;

const TOKEN: &str = "device-token";
const CODE: &str = "123456";

#[derive(Default)]
struct MockHost {
    messages: Mutex<Vec<ClientMsg>>,
    outboxes: Mutex<Vec<Outbox>>,
}

fn host_loop(rx: async_channel::Receiver<HostRequest>, mock: Arc<MockHost>) {
    while let Ok(req) = rx.recv_blocking() {
        match req {
            HostRequest::Authenticate { token, reply } => {
                let _ = reply.send((token == TOKEN).then(|| "dev-1".to_string()));
            }
            HostRequest::Pair { code, reply, .. } => {
                let _ = reply.send(if code == CODE {
                    Ok(PairResponse {
                        device_id: "dev-1".into(),
                        token: TOKEN.into(),
                        host_name: "Mac".into(),
                    })
                } else {
                    Err(PairError::Rejected)
                });
            }
            HostRequest::ListWorkspaces { reply } => {
                let _ = reply.send(vec![WorkspaceInfo {
                    id: "ws-1".into(),
                    name: "Demo".into(),
                    open: true,
                    root: "~/demo".into(),
                    dot: Some(Dot::Attention),
                    terminals: 1,
                }]);
            }
            HostRequest::ListTerminals {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(if workspace_id == "ws-1" {
                    Ok(vec![TerminalInfo {
                        tab_id: 7,
                        title: Some("shell".into()),
                        cwd: "/tmp".into(),
                        active: true,
                        dot: None,
                    }])
                } else {
                    Err("not found".into())
                });
            }
            HostRequest::Connected { out, .. } => {
                out.try_send(ServerMsg::Ready {
                    theme: ThemeWire {
                        bg: "#000000".into(),
                        fg: "#ffffff".into(),
                        cursor: "#ffffff".into(),
                    },
                    device_id: "dev-1".into(),
                    host_name: "Mac".into(),
                });
                mock.outboxes.lock().unwrap().push(out);
            }
            HostRequest::Message { msg, .. } => mock.messages.lock().unwrap().push(msg),
            HostRequest::Disconnected { .. } => {}
        }
    }
}

fn start_mock() -> (RemoteServer, Arc<MockHost>) {
    let mock = Arc::new(MockHost::default());
    let (tx, rx) = async_channel::unbounded::<HostRequest>();
    let m = mock.clone();
    thread::spawn(move || host_loop(rx, m));
    let server = RemoteServer::start(&["127.0.0.1".parse().unwrap()], 0, tx).expect("start");
    (server, mock)
}

#[test]
fn pairing_then_bearer_lists() {
    let (server, _mock) = start_mock();
    let port = server.port;
    let bad = request(port, "POST", "/pair", Some(r#"{"code":"999999"}"#), None);
    assert!(bad.starts_with("HTTP/1.1 401"), "{bad}");
    let ok = request(
        port,
        "POST",
        "/pair",
        Some(r#"{"code":"123456","label":"iPhone"}"#),
        None,
    );
    assert!(ok.contains(r#""token":"device-token""#), "{ok}");

    let anon = request(port, "GET", "/api/workspaces", None, None);
    assert!(anon.starts_with("HTTP/1.1 401"), "{anon}");
    let list = request(port, "GET", "/api/workspaces", None, Some(TOKEN));
    assert!(list.contains(r#""dot":"attention""#), "{list}");
    let terms = request(
        port,
        "GET",
        "/api/workspaces/ws-1/terminals",
        None,
        Some(TOKEN),
    );
    assert!(terms.contains(r#""tabId":7"#), "{terms}");
    let missing = request(
        port,
        "GET",
        "/api/workspaces/nope/terminals",
        None,
        Some(TOKEN),
    );
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
}

#[test]
fn responses_do_not_allow_cross_origin_reads() {
    let (server, _mock) = start_mock();
    let page = request(server.port, "GET", "/", None, None);
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(!page.contains("Access-Control-Allow-Origin"), "{page}");
    let manifest = request(server.port, "GET", "/manifest.webmanifest", None, None);
    assert!(manifest.contains("standalone"), "{manifest}");
}

#[test]
fn repeated_bad_codes_are_rate_limited() {
    let (server, _mock) = start_mock();
    for _ in 0..5 {
        let r = request(
            server.port,
            "POST",
            "/pair",
            Some(r#"{"code":"000000"}"#),
            None,
        );
        assert!(r.starts_with("HTTP/1.1 401"), "{r}");
    }
    let limited = request(
        server.port,
        "POST",
        "/pair",
        Some(r#"{"code":"123456"}"#),
        None,
    );
    assert!(limited.starts_with("HTTP/1.1 429"), "{limited}");
}

#[test]
fn websocket_hello_ready_and_forwarding() {
    let (server, mock) = start_mock();
    let url = format!("ws://127.0.0.1:{}/ws", server.port);
    let (mut ws, _) = tungstenite::connect(url).expect("connect");
    ws.send(Message::Text(
        r#"{"t":"hello","token":"device-token"}"#.into(),
    ))
    .unwrap();
    let ready = ws.read().unwrap();
    assert!(
        ready.to_text().unwrap().contains(r#""t":"ready""#),
        "{ready}"
    );

    ws.send(Message::Text(r#"{"t":"key","key":"ctrl-c"}"#.into()))
        .unwrap();
    wait_until(|| !mock.messages.lock().unwrap().is_empty());
    assert_eq!(
        mock.messages.lock().unwrap()[0],
        ClientMsg::Key {
            key: crate::protocol::NamedKey::CtrlC
        }
    );

    // Host push reaches the phone.
    mock.outboxes.lock().unwrap()[0].try_send(ServerMsg::History {
        lines: vec!["old".into()],
    });
    let pushed = ws.read().unwrap();
    assert!(pushed.to_text().unwrap().contains("old"), "{pushed}");
}

#[test]
fn websocket_bad_token_closes_unauthorized() {
    let (server, _mock) = start_mock();
    let url = format!("ws://127.0.0.1:{}/ws", server.port);
    let (mut ws, _) = tungstenite::connect(url).expect("connect");
    ws.send(Message::Text(r#"{"t":"hello","token":"nope"}"#.into()))
        .unwrap();
    match ws.read() {
        Ok(Message::Close(Some(frame))) => assert_eq!(u16::from(frame.code), 4401),
        other => panic!("expected close, got {other:?}"),
    }
}

#[test]
fn websocket_rejects_foreign_origin() {
    let (server, _mock) = start_mock();
    let port = server.port;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .write_all(
            format!(
                "GET /ws HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://evil.example\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
            )
            .as_bytes(),
        )
        .unwrap();
    let mut out = String::new();
    let _ = stream.read_to_string(&mut out);
    assert!(out.starts_with("HTTP/1.1 403"), "{out}");
}

#[test]
fn stopped_server_stops_accepting() {
    let (server, _mock) = start_mock();
    let port = server.port;
    server.stop();
    thread::sleep(Duration::from_millis(80));
    if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
        stream
            .set_read_timeout(Some(Duration::from_millis(300)))
            .ok();
        let _ = stream.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n");
        let mut body = String::new();
        let _ = stream.read_to_string(&mut body);
        assert!(body.is_empty(), "{body}");
    }
}

fn wait_until(mut f: impl FnMut() -> bool) {
    for _ in 0..100 {
        if f() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("condition not reached");
}

fn request(port: u16, method: &str, path: &str, body: Option<&str>, token: Option<&str>) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let body = body.unwrap_or("");
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{auth}Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    String::from_utf8_lossy(&raw).into_owned()
}

#[test]
fn bad_tokens_never_lock_out_paired_phones() {
    let (server, _mock) = start_mock();
    for _ in 0..30 {
        let r = request(server.port, "GET", "/api/workspaces", None, Some("guess"));
        assert!(r.starts_with("HTTP/1.1 401"), "{r}");
    }
    let ok = request(server.port, "GET", "/api/workspaces", None, Some(TOKEN));
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
}

#[test]
fn oversized_body_is_refused() {
    let (server, _mock) = start_mock();
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let _ =
        stream.write_all(b"POST /pair HTTP/1.1\r\nHost: x\r\nContent-Length: 999999999\r\n\r\n{");
    let mut out = String::new();
    let _ = stream.read_to_string(&mut out);
    assert!(!out.contains("200"), "{out}");
}

#[test]
fn busy_port_is_reported_and_freed_on_drop() {
    let (server, _mock) = start_mock();
    let port = server.port;
    let (tx, _rx) = async_channel::unbounded::<HostRequest>();
    let second = RemoteServer::start(&["127.0.0.1".parse().unwrap()], port, tx.clone());
    assert!(matches!(second, Err(StartError::PortBusy)));
    drop(server);
    // Drop joined the listener: the port is free immediately.
    let third = RemoteServer::start(&["127.0.0.1".parse().unwrap()], port, tx);
    assert!(third.is_ok());
}
