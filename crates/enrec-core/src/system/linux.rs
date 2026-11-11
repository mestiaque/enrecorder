//! Linux readers for [`super::SystemInfo`].

use super::display::{desktop_from, detect_from};
use super::SystemInfo;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

fn os_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_else(|| "Linux".into())
}

/// A socket in `$XDG_RUNTIME_DIR` that accepts a connection (the server is really running,
/// not just a stale file).
fn socket_answers(rel: &str) -> bool {
    let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") else { return false };
    UnixStream::connect(PathBuf::from(dir).join(rel)).is_ok()
}

pub fn detect() -> SystemInfo {
    let mut render_nodes: Vec<String> = std::fs::read_dir("/dev/dri")
        .map(|d| d.flatten().filter_map(|e| e.file_name().to_str().filter(|n| n.starts_with("renderD")).map(|n| format!("/dev/dri/{n}"))).collect())
        .unwrap_or_default();
    render_nodes.sort();
    SystemInfo {
        os: "linux".into(),
        os_name: os_name(),
        arch: std::env::consts::ARCH.into(),
        display_server: detect_from(|k| std::env::var(k).ok()),
        desktop: desktop_from(std::env::var("XDG_CURRENT_DESKTOP").ok()),
        pipewire: Some(socket_answers("pipewire-0")),
        pulse: Some(socket_answers("pulse/native")),
        render_nodes,
    }
}
