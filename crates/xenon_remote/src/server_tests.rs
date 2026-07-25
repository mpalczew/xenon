//! Integration tests for the lean remote HTTP stack.

use super::*;
use crate::protocol::{TerminalInfo, WorkspaceInfo};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

struct MockHost {
    injects: Mutex<Vec<(u64, String)>>,
    frames: AtomicU64,
}

fn host_loop(rx: async_channel::Receiver<HostRequest>, mock: Arc<MockHost>) {
    while let Ok(req) = rx.recv_blocking() {
        match req {
            HostRequest::ListWorkspaces { reply } => {
                let _ = reply.send(vec![WorkspaceInfo {
                    id: "ws-1".into(),
                    name: "Demo".into(),
                    open: true,
                    root: "/tmp/demo".into(),
                }]);
            }
            HostRequest::ListTerminals {
                workspace_id,
                reply,
            } => {
                if workspace_id == "ws-1" {
                    let _ = reply.send(Ok(vec![TerminalInfo {
                        tab_id: 7,
                        title: Some("shell".into()),
                        cwd: "/tmp".into(),
                        active: true,
                    }]));
                } else {
                    let _ = reply.send(Err("not found".into()));
                }
            }
            HostRequest::CaptureFrame { tab_id, reply, .. } => {
                let seq = mock.frames.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = reply.send(Ok(ViewportSnapshot {
                    tab_id,
                    seq,
                    cols: 8,
                    rows: 2,
                    lines: vec!["hello".into(), "world".into()],
                }));
            }
            HostRequest::Inject {
                tab_id,
                text,
                reply,
                ..
            } => {
                mock.injects
                    .lock()
                    .expect("inject lock")
                    .push((tab_id, text));
                let _ = reply.send(Ok(()));
            }
        }
    }
}

fn start_mock() -> (
    RemoteServer,
    Arc<MockHost>,
    async_channel::Sender<HostRequest>,
) {
    let mock = Arc::new(MockHost {
        injects: Mutex::new(Vec::new()),
        frames: AtomicU64::new(0),
    });
    let (tx, rx) = async_channel::unbounded::<HostRequest>();
    let m = mock.clone();
    thread::spawn(move || host_loop(rx, m));
    let server = RemoteServer::start(0, crate::auth::new_token(), tx.clone()).expect("start");
    thread::sleep(Duration::from_millis(50));
    (server, mock, tx)
}

fn auth_ticket(port: u16, token: &str) -> String {
    let ok = http_post(
        &format!("http://127.0.0.1:{port}/auth"),
        &format!(r#"{{"token":"{token}"}}"#),
    );
    assert!(ok.contains("ticket"), "{ok}");
    ok.split("\"ticket\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("ticket")
        .to_string()
}

#[test]
fn http_lean_stack_auth_list_frame_inject() {
    let (server, mock, tx) = start_mock();
    let port = server.port;
    let token = server.token.clone();

    let bad = http_post(
        &format!("http://127.0.0.1:{port}/auth"),
        r#"{"token":"nope"}"#,
    );
    assert!(bad.contains("401") || bad.contains("unauthorized"), "{bad}");

    let ticket = auth_ticket(port, &token);

    let list = http_get_auth(&format!("http://127.0.0.1:{port}/api/workspaces"), &ticket);
    assert!(list.contains("Demo"), "{list}");

    let terms = http_get_auth(
        &format!("http://127.0.0.1:{port}/api/workspaces/ws-1/terminals"),
        &ticket,
    );
    assert!(terms.contains("\"tabId\":7"), "{terms}");

    // PNG frame (binary-safe)
    let frame = raw_request_bytes(
        "GET",
        &format!("http://127.0.0.1:{port}/api/frame?workspaceId=ws-1&tabId=7&since=0"),
        None,
        Some(&ticket),
    );
    let head = String::from_utf8_lossy(&frame[..frame.len().min(400)]);
    assert!(head.contains("200") || head.contains("image/png"), "{head}");
    assert!(head.to_ascii_lowercase().contains("x-xenon-seq:"), "{head}");
    assert!(
        frame.windows(4).any(|w| w == [0x89, b'P', b'N', b'G']),
        "missing PNG signature"
    );

    let inj = raw_request(
        "POST",
        &format!("http://127.0.0.1:{port}/api/inject"),
        Some(r#"{"workspaceId":"ws-1","tabId":7,"text":"echo hi\n"}"#),
        Some(&ticket),
    );
    assert!(inj.contains("200") || inj.contains("\"ok\""), "{inj}");
    assert!(
        mock.injects
            .lock()
            .unwrap()
            .iter()
            .any(|(id, t)| *id == 7 && t == "echo hi\n")
    );

    let page = http_get(&format!("http://127.0.0.1:{port}/"));
    assert!(
        page.contains("Xenon remote") || page.contains("/api/frame"),
        "{page}"
    );
    assert!(page.contains("<img"), "{page}");
    assert!(
        !page.contains("WebSocket") && !page.contains("ws://"),
        "{page}"
    );

    server.stop();
    drop(tx);
}

#[test]
fn stopped_server_not_usable() {
    let (server, _mock, tx) = start_mock();
    let port = server.port;
    let page = http_get(&format!("http://127.0.0.1:{port}/"));
    assert!(page.contains("200") || page.contains("Xenon"), "{page}");
    server.stop();
    thread::sleep(Duration::from_millis(80));
    match TcpStream::connect(format!("127.0.0.1:{port}")) {
        Err(_) => {}
        Ok(mut stream) => {
            stream
                .set_read_timeout(Some(Duration::from_millis(300)))
                .ok();
            let _ = stream.write_all(
                format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            );
            let mut body = String::new();
            let _ = stream.read_to_string(&mut body);
            assert!(body.is_empty() || !body.contains("ticket"), "{body}");
        }
    }
    drop(tx);
}

fn http_post(url: &str, body: &str) -> String {
    raw_request("POST", url, Some(body), None)
}
fn http_get(url: &str) -> String {
    raw_request("GET", url, None, None)
}
fn http_get_auth(url: &str, ticket: &str) -> String {
    raw_request("GET", url, None, Some(ticket))
}

fn raw_request(method: &str, url: &str, body: Option<&str>, ticket: Option<&str>) -> String {
    String::from_utf8_lossy(&raw_request_bytes(method, url, body, ticket)).into_owned()
}

fn raw_request_bytes(method: &str, url: &str, body: Option<&str>, ticket: Option<&str>) -> Vec<u8> {
    let url = url.strip_prefix("http://").unwrap();
    let (hostport, path) = url
        .split_once('/')
        .map(|(h, p)| (h, format!("/{p}")))
        .unwrap();
    let mut stream = TcpStream::connect(hostport).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let body = body.unwrap_or("");
    let auth = ticket
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {hostport}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{auth}Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }
    raw
}
