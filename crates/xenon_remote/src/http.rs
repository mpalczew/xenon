use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};

use anyhow::{Result, bail};
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};

// Parsed here with httparse rather than a server crate: tiny_http (checked
// at 0.12) has no header size cap, connection cap or read timeout, and this
// port reads requests before it checks the device token.
const MAX_HEAD: usize = 16 * 1024;
const MAX_HEADERS: usize = 64;
pub(crate) const MAX_BODY: usize = 8 * 1024;

pub(crate) struct Request {
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
    pub(crate) peer: IpAddr,
}

impl Request {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }

    pub(crate) fn content_length(&self) -> usize {
        self.header("content-length")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }
}

/// Reads the request line and headers; returns them with any body bytes read past them.
pub(crate) fn read_head(stream: &mut TcpStream, peer: IpAddr) -> Result<Option<Request>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
        let mut slots = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut parsed = httparse::Request::new(&mut slots);
        if let httparse::Status::Complete(len) = parsed.parse(&buf)? {
            let target = parsed.path.unwrap_or("/");
            let path = target.split('?').next().unwrap_or("/").to_string();
            let method = Method::from_bytes(parsed.method.unwrap_or("").as_bytes())?;
            let mut headers = HeaderMap::new();
            for h in parsed.headers.iter() {
                headers.append(
                    HeaderName::from_bytes(h.name.as_bytes())?,
                    HeaderValue::from_bytes(h.value)?,
                );
            }
            return Ok(Some(Request {
                method,
                path,
                headers,
                body: buf.split_off(len),
                peer,
            }));
        }
        if buf.len() > MAX_HEAD {
            bail!("http headers too large");
        }
    }
}

/// Completes `req.body` up to its Content-Length.
pub(crate) fn read_body(req: &mut Request, stream: &mut TcpStream) -> Result<()> {
    let len = req.content_length();
    req.body.truncate(len);
    let rest = (len - req.body.len()) as u64;
    Read::by_ref(stream).take(rest).read_to_end(&mut req.body)?;
    Ok(())
}

pub(crate) fn write_http(
    stream: &mut TcpStream,
    status: u16,
    ctype: &str,
    body: &[u8],
) -> Result<()> {
    let status = StatusCode::from_u16(status)?;
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

pub(crate) fn write_json_error(stream: &mut TcpStream, status: u16, msg: &str) -> Result<()> {
    let body = serde_json::json!({ "error": msg }).to_string();
    write_http(stream, status, "application/json", body.as_bytes())
}

pub(crate) fn write_json<T: serde::Serialize>(stream: &mut TcpStream, value: &T) -> Result<()> {
    write_http(stream, 200, "application/json", &serde_json::to_vec(value)?)
}
