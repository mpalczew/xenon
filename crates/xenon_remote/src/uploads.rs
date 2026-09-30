//! Images the phone attaches for the agent. Saved under one directory with
//! server-chosen names; anything older than a week is pruned on each save.

use std::fs;
use std::io;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::http;
use crate::server::{self, Request, Shared};

/// Largest image accepted (full-resolution iPhone photos fit).
pub(crate) const MAX_UPLOAD: usize = 25 * 1024 * 1024;
const KEEP: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// `POST /api/uploads`: raw image body in, `{"path": …}` out.
pub(crate) fn serve(stream: &mut TcpStream, req: &Request, shared: &Shared) -> Result<()> {
    match shared.authenticate(server::bearer(&req.headers)) {
        Ok(Some(_)) => {}
        Ok(None) => return http::write_json_error(stream, 401, "unauthorized"),
        Err(e) => return http::write_json_error(stream, 503, &e),
    }
    let Some(dir) = shared.uploads_dir.get() else {
        return http::write_json_error(stream, 503, "uploads are off");
    };
    let content_type = req.headers.get("content-type").map_or("", String::as_str);
    let Some(ext) = extension(content_type) else {
        return http::write_json_error(stream, 415, "send a PNG, JPEG, GIF, WebP, or HEIC image");
    };
    if req.body.is_empty() {
        return http::write_json_error(stream, 400, "empty image");
    }
    match save(dir, ext, &req.body) {
        Ok(path) => http::write_http(
            stream,
            200,
            "application/json",
            serde_json::json!({ "path": path }).to_string().as_bytes(),
        ),
        Err(e) => http::write_json_error(stream, 500, &format!("couldn’t save: {e}")),
    }
}

/// File extension for an accepted image type; `None` rejects the upload.
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

/// Write `bytes` to a fresh file in `dir` and return its path.
pub(crate) fn save(dir: &Path, ext: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    prune(dir, SystemTime::now());
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_images_only() {
        assert_eq!(extension("image/png"), Some("png"));
        assert_eq!(extension("IMAGE/JPEG; charset=binary"), Some("jpg"));
        assert_eq!(extension("text/html"), None);
        assert_eq!(extension(""), None);
    }

    #[test]
    fn saves_and_prunes_week_old_files() {
        let dir = std::env::temp_dir().join(format!("xenon-uploads-{}", uuid::Uuid::new_v4()));
        let path = save(&dir, "png", b"img").expect("save");
        assert_eq!(fs::read(&path).unwrap(), b"img");
        assert!(path.starts_with(&dir));
        prune(&dir, SystemTime::now() + KEEP + Duration::from_secs(1));
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
