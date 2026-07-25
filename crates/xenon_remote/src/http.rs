//! Minimal HTTP parse/write for the mobile remote server.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

use anyhow::{Result, anyhow};

use crate::host::ViewportSnapshot;

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

pub(crate) fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?;
        let v = it.next().unwrap_or("");
        if k == key {
            return Some(v);
        }
    }
    None
}

pub(crate) fn extract_body(
    first: &[u8],
    body_start: usize,
    headers: &HashMap<String, String>,
    stream: &mut TcpStream,
) -> Result<Vec<u8>> {
    let clen = headers
        .get("content-length")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = Vec::new();
    if body_start < first.len() {
        body.extend_from_slice(&first[body_start..]);
    }
    while body.len() < clen {
        let mut chunk = [0u8; 4096];
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
        404 => "Not Found",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

pub(crate) fn write_http_status(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    extra: &[(&str, &str)],
) -> Result<()> {
    let mut header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\n"
    );
    for (k, v) in extra {
        header.push_str(k);
        header.push_str(": ");
        header.push_str(v);
        header.push_str("\r\n");
    }
    header.push_str("\r\n");
    stream.write_all(header.as_bytes())?;
    Ok(())
}

pub(crate) fn write_png_frame(
    stream: &mut TcpStream,
    snap: &ViewportSnapshot,
    png: &[u8],
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nX-Xenon-Seq: {}\r\nX-Xenon-TabId: {}\r\nX-Xenon-Cols: {}\r\nX-Xenon-Rows: {}\r\n\r\n",
        png.len(),
        snap.seq,
        snap.tab_id,
        snap.cols,
        snap.rows
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(png)?;
    Ok(())
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
