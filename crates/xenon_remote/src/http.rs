//! Minimal HTTP parse/write for the mobile remote server.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

use anyhow::{Result, anyhow};

pub(crate) fn split_http(head: &str) -> Result<(&str, HashMap<String, String>, usize)> {
    let header_end = head
        .find("\r\n\r\n")
        .or_else(|| head.find("\n\n"))
        .ok_or_else(|| anyhow!("incomplete http headers"))?;
    let sep_len = if head[header_end..].starts_with("\r\n\r\n") {
        4
    } else {
        2
    };
    let header_block = &head[..header_end];
    let mut lines = header_block.split('\n');
    let req_line = lines.next().unwrap_or("").trim_end_matches('\r');
    let mut headers = HashMap::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    Ok((req_line, headers, header_end + sep_len))
}

pub(crate) fn split_path_query(path_q: &str) -> (&str, &str) {
    match path_q.split_once('?') {
        Some((p, q)) => (p, q),
        None => (path_q, ""),
    }
}

/// Largest request body accepted outside uploads (pairing JSON is tiny).
pub(crate) const MAX_BODY: usize = 8 * 1024;

pub(crate) fn content_length(headers: &HashMap<String, String>) -> usize {
    headers
        .get("content-length")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
}

/// Body bytes already in `first` plus the rest read from `stream`, up to
/// `clen` (callers check `clen` against their limit first).
pub(crate) fn extract_body(
    first: &[u8],
    body_start: usize,
    clen: usize,
    stream: &mut TcpStream,
) -> Result<Vec<u8>> {
    let mut body = Vec::with_capacity(clen);
    if body_start < first.len() {
        body.extend_from_slice(&first[body_start..]);
    }
    let mut chunk = vec![0u8; 64 * 1024];
    while body.len() < clen {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(clen);
    Ok(body)
}

pub(crate) fn write_http(
    stream: &mut TcpStream,
    status: u16,
    ctype: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

/// `101 Switching Protocols` for a WebSocket upgrade.
pub(crate) fn write_ws_accept(stream: &mut TcpStream, accept_key: &str) -> Result<()> {
    let header = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept_key}\r\n\r\n"
    );
    stream.write_all(header.as_bytes())?;
    Ok(())
}

pub(crate) fn write_json_error(stream: &mut TcpStream, status: u16, msg: &str) -> Result<()> {
    let body = serde_json::json!({ "error": msg }).to_string();
    write_http(stream, status, "application/json", body.as_bytes())
}

pub(crate) fn urlencoding_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                if let Some(hex) = h
                    && let Ok(v) = u8::from_str_radix(hex, 16)
                {
                    out.push(v as char);
                    i += 3;
                    continue;
                }
                out.push('%');
                i += 1;
            }
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}
