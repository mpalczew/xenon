use std::fs;
use std::io;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;

use crate::remote::http::{self, Request};
use crate::remote::labels::unix_now;
use crate::remote::server::Shared;

pub(crate) const MAX_UPLOAD: usize = 25 * 1024 * 1024;
const KEEP: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub(crate) fn serve(
    stream: &mut TcpStream,
    req: &Request,
    shared: &Shared,
    auth: Result<Option<String>, String>,
) -> Result<()> {
    match auth {
        Ok(Some(_)) => {}
        Ok(None) => return http::write_json_error(stream, 401, "unauthorized"),
        Err(e) => return http::write_json_error(stream, 503, &e),
    }
    let content_type = req.header("content-type").unwrap_or("");
    let Some(ext) = extension(content_type) else {
        return http::write_json_error(stream, 415, "send a PNG, JPEG, GIF, WebP, or HEIC image");
    };
    if req.body.is_empty() {
        return http::write_json_error(stream, 400, "empty image");
    }
    match save(&shared.uploads_dir, ext, &req.body) {
        Ok(path) => http::write_json(stream, &serde_json::json!({ "path": path })),
        Err(e) => http::write_json_error(stream, 500, &format!("couldn’t save: {e}")),
    }
}

pub(crate) fn extension(content_type: &str) -> Option<&'static str> {
    let mime = content_type.split(';').next().unwrap_or("").trim();
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/heic" => Some("heic"),
        _ => None,
    }
}

pub(crate) fn save(dir: &Path, ext: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    prune(dir, SystemTime::now());
    let secs = unix_now();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let path = dir.join(format!("phone-{secs}-{}.{ext}", &id[..8]));
    fs::write(&path, bytes)?;
    Ok(path)
}

fn prune(dir: &Path, now: SystemTime) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age > KEEP);
        if old {
            let _ = fs::remove_file(entry.path());
        }
    }
}
