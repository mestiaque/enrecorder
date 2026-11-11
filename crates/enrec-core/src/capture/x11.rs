//! X11 backend. Sources come from the X server itself: monitors from RandR, windows from the
//! window manager's EWMH client list. Capture uses FFmpeg's `x11grab` device (XCB + XShm),
//! which copies pixels straight from the X server into the encoder process.
//!
//! Known X11 limits (documented in FEATURES.md): a captured *window* is grabbed from the screen,
//! so other windows covering it are captured too, and a window that is resized keeps the size
//! it had when recording started.

use super::{even, BackendKind, CaptureBackend, CaptureInput, CaptureRequest, CaptureSource};
use crate::error::{AppError, Result};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, Window};
use x11rb::rust_connection::RustConnection;

pub struct X11Backend {
    /// `DISPLAY`, e.g. ":0".
    display: String,
}

impl X11Backend {
    pub fn new() -> Result<Self> {
        let display = std::env::var("DISPLAY").ok().filter(|d| !d.trim().is_empty()).ok_or_else(|| AppError::Unsupported("no X11 display (DISPLAY is not set)".into()))?;
        Ok(Self { display })
    }

    fn connect(&self) -> Result<(RustConnection, usize)> {
        x11rb::connect(Some(&self.display)).map_err(|e| AppError::Unsupported(format!("cannot connect to the X server {}: {e}", self.display)))
    }
}

fn x_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("X11: {e}"))
}

fn atom(c: &RustConnection, name: &str) -> Result<Atom> {
    Ok(c.intern_atom(false, name.as_bytes()).map_err(x_err)?.reply().map_err(x_err)?.atom)
}

fn prop_u32(c: &RustConnection, w: Window, prop: Atom, ty: impl Into<Atom>) -> Vec<u32> {
    c.get_property(false, w, prop, ty, 0, 4096)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .and_then(|r| r.value32().map(|v| v.collect()))
        .unwrap_or_default()
}

fn prop_string(c: &RustConnection, w: Window, prop: Atom, ty: impl Into<Atom>) -> String {
    c.get_property(false, w, prop, ty, 0, 1024)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|r| String::from_utf8_lossy(&r.value).trim_end_matches('\0').to_string())
        .unwrap_or_default()
}

struct Snapshot {
    root_w: u32,
    root_h: u32,
    monitors: Vec<CaptureSource>,
    windows: Vec<CaptureSource>,
}

impl X11Backend {
    fn snapshot(&self) -> Result<Snapshot> {
        let (c, screen_num) = self.connect()?;
        let screen = &c.setup().roots[screen_num];
        let root = screen.root;
        let (root_w, root_h) = (screen.width_in_pixels as u32, screen.height_in_pixels as u32);

        let mut monitors = Vec::new();
        if let Ok(reply) = c.randr_get_monitors(root, true).map_err(x_err).and_then(|ck| ck.reply().map_err(x_err)) {
            for m in reply.monitors {
                let name = c.get_atom_name(m.name).ok().and_then(|ck| ck.reply().ok()).map(|r| String::from_utf8_lossy(&r.name).into_owned()).unwrap_or_else(|| format!("Monitor {}", monitors.len() + 1));
                monitors.push(CaptureSource::Monitor { id: name.clone(), name, width: m.width as u32, height: m.height as u32, x: m.x as i32, y: m.y as i32, primary: m.primary });
            }
        }

        let net_client_list = atom(&c, "_NET_CLIENT_LIST")?;
        let net_wm_name = atom(&c, "_NET_WM_NAME")?;
        let utf8 = atom(&c, "UTF8_STRING")?;
        let net_wm_pid = atom(&c, "_NET_WM_PID")?;
        let net_wm_type = atom(&c, "_NET_WM_WINDOW_TYPE")?;
        let skip_types = [atom(&c, "_NET_WM_WINDOW_TYPE_DOCK")?, atom(&c, "_NET_WM_WINDOW_TYPE_DESKTOP")?];
        let me = std::process::id();
        let mut windows = Vec::new();
        for w in prop_u32(&c, root, net_client_list, AtomEnum::WINDOW) {
            if prop_u32(&c, w, net_wm_pid, AtomEnum::CARDINAL).first() == Some(&me) {
                continue; // ENRecorder's own windows
            }
            if prop_u32(&c, w, net_wm_type, AtomEnum::ATOM).iter().any(|t| skip_types.contains(t)) {
                continue;
            }
            let mut title = prop_string(&c, w, net_wm_name, utf8);
            if title.is_empty() {
                title = prop_string(&c, w, AtomEnum::WM_NAME.into(), AtomEnum::STRING);
            }
            // WM_CLASS is "instance\0Class\0"; the class is the readable app name.
            let class = prop_string(&c, w, AtomEnum::WM_CLASS.into(), AtomEnum::STRING);
            let app = class.split('\0').rfind(|s| !s.is_empty()).unwrap_or("").to_string();
            let Ok(geo) = c.get_geometry(w).map_err(x_err).and_then(|ck| ck.reply().map_err(x_err)) else { continue };
            if geo.width < 32 || geo.height < 32 || title.is_empty() {
                continue;
            }
            windows.push(CaptureSource::Window { id: w.to_string(), title, app, width: geo.width as u32, height: geo.height as u32 });
        }
        Ok(Snapshot { root_w, root_h, monitors, windows })
    }

    fn grab_args(&self, fps: u32, cursor: bool) -> Vec<String> {
        vec!["-f".into(), "x11grab".into(), "-framerate".into(), fps.to_string(), "-draw_mouse".into(), if cursor { "1" } else { "0" }.into()]
    }

    /// `-video_size WxH -i :0+X,Y` for a rectangle, clamped to the screen and made even.
    fn rect_input(&self, req: &CaptureRequest, root: (u32, u32), x: i32, y: i32, w: u32, h: u32) -> Result<CaptureInput> {
        let x = x.clamp(0, root.0 as i32 - 2) as u32;
        let y = y.clamp(0, root.1 as i32 - 2) as u32;
        let w = even(w.min(root.0 - x));
        let h = even(h.min(root.1 - y));
        if w < 16 || h < 16 {
            return Err(AppError::InvalidSetting("the area to record is too small".into()));
        }
        let mut args = self.grab_args(req.fps, req.show_cursor);
        args.extend(["-video_size".into(), format!("{w}x{h}"), "-i".into(), format!("{}+{x},{y}", self.display)]);
        Ok(CaptureInput { args, width: w, height: h })
    }
}

impl CaptureBackend for X11Backend {
    fn kind(&self) -> BackendKind {
        BackendKind::X11
    }

    fn list_sources(&self) -> Result<Vec<CaptureSource>> {
        let s = self.snapshot()?;
        let mut v = vec![CaptureSource::Desktop { width: s.root_w, height: s.root_h }];
        v.extend(s.monitors);
        v.extend(s.windows);
        Ok(v)
    }

    fn input(&self, req: &CaptureRequest) -> Result<CaptureInput> {
        if !(1..=120).contains(&req.fps) {
            return Err(AppError::InvalidSetting(format!("{} fps is not supported", req.fps)));
        }
        let s = self.snapshot()?;
        let root = (s.root_w, s.root_h);
        match &req.source {
            CaptureSource::Desktop { .. } => self.rect_input(req, root, 0, 0, s.root_w, s.root_h),
            CaptureSource::Region { x, y, width, height } => self.rect_input(req, root, *x, *y, *width, *height),
            CaptureSource::Monitor { id, .. } => {
                // Look the monitor up again: it may have moved or been unplugged.
                let m = s.monitors.iter().find_map(|m| match m {
                    CaptureSource::Monitor { id: mid, x, y, width, height, .. } if mid == id => Some((*x, *y, *width, *height)),
                    _ => None,
                });
                let (x, y, w, h) = m.ok_or_else(|| AppError::NotFound(format!("monitor {id} is no longer connected")))?;
                self.rect_input(req, root, x, y, w, h)
            }
            CaptureSource::Window { id, .. } => {
                let found = s.windows.iter().find_map(|w| match w {
                    CaptureSource::Window { id: wid, width, height, .. } if wid == id => Some((*width, *height)),
                    _ => None,
                });
                let (w, h) = found.ok_or_else(|| AppError::NotFound("the window was closed".into()))?;
                let mut args = self.grab_args(req.fps, req.show_cursor);
                args.extend(["-window_id".into(), id.clone(), "-i".into(), self.display.clone()]);
                Ok(CaptureInput { args, width: w, height: h })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs against the real X server when there is one (skipped in headless CI).
    #[test]
    fn real_x_server_if_available() {
        let Ok(b) = X11Backend::new() else { return };
        let Ok(sources) = b.list_sources() else { return };
        assert!(matches!(sources[0], CaptureSource::Desktop { width, height } if width > 0 && height > 0));
        let req = CaptureRequest { source: sources[0].clone(), fps: 30, show_cursor: true };
        let input = b.input(&req).unwrap();
        assert!(input.args.windows(2).any(|w| w == ["-f", "x11grab"]));
        assert_eq!(input.width % 2, 0);
        // A region partly off-screen is clamped and made even.
        let req = CaptureRequest { source: CaptureSource::Region { x: -50, y: 10, width: 101, height: 99 }, fps: 30, show_cursor: false };
        let input = b.input(&req).unwrap();
        assert_eq!((input.width, input.height), (100, 98));
        assert!(input.args.iter().any(|a| a.ends_with("+0,10")));
        assert!(b.input(&CaptureRequest { fps: 0, ..req }).is_err());
    }
}
