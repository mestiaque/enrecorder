//! Preview server for the library: WebKitGTK's media player (GStreamer) cannot read Tauri's
//! custom `asset:` scheme, so videos and audio are served over HTTP on 127.0.0.1 only, on a
//! random port, under a random per-run token, for library items only, with Range support
//! (seeking). Nothing listens on other interfaces.

use crate::state::AppState;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

static SERVER: OnceLock<(u16, String)> = OnceLock::new();

fn token() -> String {
    // 128 random bits from the OS through uuid v4.
    enrec_core::database::new_id().replace('-', "")
}

pub fn start(app: AppHandle) {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        log::warn!("preview server could not start");
        return;
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let _ = SERVER.set((port, token()));
    std::thread::Builder::new()
        .name("preview-server".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let app = app.clone();
                std::thread::spawn(move || {
                    let _ = serve(&app, stream);
                });
            }
        })
        .ok();
}

/// URL of a library item for <video>/<audio>.
pub fn url(id: &str) -> Option<String> {
    SERVER.get().map(|(port, tok)| format!("http://127.0.0.1:{port}/m/{tok}/{id}"))
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "gif" => "image/gif",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "opus" | "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

fn reply(s: &mut TcpStream, code: &str) -> std::io::Result<()> {
    write!(s, "HTTP/1.1 {code}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

fn serve(app: &AppHandle, mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let mut range: Option<String> = None;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.eq_ignore_ascii_case("range") {
                range = Some(v.trim().to_string());
            }
        }
    }
    log::debug!("preview request: {} range={range:?}", first.trim());
    let mut parts = first.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let head = method == "HEAD";
    if method != "GET" && !head {
        return reply(&mut stream, "405 Method Not Allowed");
    }
    // "?v=…" only defeats caching in the player.
    let target = target.split('?').next().unwrap_or("");
    let Some((port_tok, id)) = target.strip_prefix("/m/").and_then(|r| r.split_once('/')) else { return reply(&mut stream, "404 Not Found") };
    match SERVER.get() {
        Some((_, tok)) if tok == port_tok => {}
        _ => return reply(&mut stream, "403 Forbidden"),
    }
    let item = match app.state::<AppState>().db.media_item(id) {
        Ok(i) => i,
        Err(_) => return reply(&mut stream, "404 Not Found"),
    };
    let path = Path::new(&item.path);
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return reply(&mut stream, "404 Not Found"),
    };
    let len = file.metadata()?.len();
    let (start, end) = match range.as_deref().and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.split_once('-')) {
        Some((a, b)) => {
            let start = a.parse::<u64>().unwrap_or(0);
            let end = b.parse::<u64>().ok().filter(|e| *e < len).unwrap_or(len.saturating_sub(1));
            if start >= len || start > end {
                write!(stream, "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{len}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
                return Ok(());
            }
            (start, end)
        }
        None => (0, len.saturating_sub(1)),
    };
    let partial = range.is_some();
    log::info!("preview {} {}-{} of {len}", item.title, start, end);
    let count = if len == 0 { 0 } else { end - start + 1 };
    let status = if partial { "206 Partial Content" } else { "200 OK" };
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {}\r\nAccept-Ranges: bytes\r\nContent-Length: {count}\r\nCache-Control: no-store\r\nConnection: close\r\n", mime(path))?;
    if partial {
        write!(stream, "Content-Range: bytes {start}-{end}/{len}\r\n")?;
    }
    write!(stream, "\r\n")?;
    if head || count == 0 {
        return Ok(());
    }
    file.seek(SeekFrom::Start(start))?;
    let mut left = count;
    let mut buf = vec![0u8; 256 * 1024];
    while left > 0 {
        let want = buf.len().min(left as usize);
        let n = file.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        // The player closes connections it no longer needs (seeking): that ends the loop.
        if stream.write_all(&buf[..n]).is_err() {
            break;
        }
        left -= n as u64;
    }
    Ok(())
}
