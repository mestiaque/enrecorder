//! Wayland backend: xdg-desktop-portal ScreenCast + PipeWire.
//!
//! On Wayland an app cannot read the screen by itself. ENRecorder asks the desktop's portal for
//! a screen cast; the desktop shows its own dialog where the user picks a monitor or a window
//! (or cancels). The portal then hands over a PipeWire stream, whose frames go through
//! [`super::frames`] to FFmpeg. Works with any desktop that implements the portal (GNOME, KDE,
//! wlroots compositors); tested end to end with sway + xdg-desktop-portal-wlr.

use super::frames::{FrameSlot, FrameSource, PixelOrder};
use super::{even, BackendKind, CaptureBackend, CaptureInput, CaptureRequest, CaptureSource};
use crate::error::{AppError, Result};
use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::PersistMode;
use std::os::fd::OwnedFd;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

pub struct WaylandBackend;

impl WaylandBackend {
    pub fn new() -> Result<Self> {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return Err(AppError::Unsupported("no Wayland display (WAYLAND_DISPLAY is not set)".into()));
        }
        Ok(Self)
    }
}

/// What the portal granted.
struct Granted {
    fd: OwnedFd,
    node_id: u32,
    size: Option<(u32, u32)>,
}

/// Keeps the portal session open (the share ends when it closes) on its own thread, because the
/// session borrows the portal proxy and both live inside an async runtime.
struct PortalShare {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PortalShare {
    fn open(show_cursor: bool) -> Result<(Self, Granted)> {
        let (res_tx, res_rx) = mpsc::channel::<Result<Granted>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("screencast-portal".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = res_tx.send(Err(AppError::Other(format!("could not start the portal client: {e}"))));
                        return;
                    }
                };
                rt.block_on(async move {
                    let proxy = match Screencast::new().await {
                        Ok(p) => p,
                        Err(e) => {
                            let _ = res_tx.send(Err(AppError::Unsupported(format!("the screen sharing portal is not available ({e}). Install xdg-desktop-portal and the portal for your desktop."))));
                            return;
                        }
                    };
                    let session = match proxy.create_session().await {
                        Ok(s) => s,
                        Err(e) => {
                            let _ = res_tx.send(Err(portal_err(e)));
                            return;
                        }
                    };
                    let cursor = match proxy.available_cursor_modes().await {
                        Ok(modes) if show_cursor && modes.contains(CursorMode::Embedded) => CursorMode::Embedded,
                        Ok(modes) if !show_cursor && modes.contains(CursorMode::Hidden) => CursorMode::Hidden,
                        Ok(modes) if modes.contains(CursorMode::Embedded) => CursorMode::Embedded,
                        _ => CursorMode::Hidden,
                    };
                    let granted = async {
                        proxy.select_sources(&session, cursor, SourceType::Monitor | SourceType::Window, false, None, PersistMode::DoNot).await.map_err(portal_err)?;
                        let response = proxy.start(&session, None).await.map_err(portal_err)?.response().map_err(portal_err)?;
                        let stream = response.streams().first().cloned().ok_or_else(|| AppError::Other("the portal returned no stream".into()))?;
                        let fd = proxy.open_pipe_wire_remote(&session).await.map_err(portal_err)?;
                        let size = stream.size().map(|(w, h)| (w.max(0) as u32, h.max(0) as u32)).filter(|(w, h)| *w > 0 && *h > 0);
                        Ok::<_, AppError>(Granted { fd, node_id: stream.pipe_wire_node_id(), size })
                    }
                    .await;
                    let ok = granted.is_ok();
                    let _ = res_tx.send(granted);
                    if ok {
                        // Hold the session until the recording ends.
                        let _ = tokio::task::spawn_blocking(move || stop_rx.recv()).await;
                    }
                    let _ = session.close().await;
                });
            })
            .map_err(|e| AppError::Other(format!("could not start the portal thread: {e}")))?;
        // The user may take a while in the dialog.
        let granted = res_rx.recv_timeout(Duration::from_secs(300)).map_err(|_| AppError::Other("no answer from the screen sharing dialog".into()))??;
        Ok((Self { stop: Some(stop_tx), thread: Some(thread) }, granted))
    }

    fn close(&mut self) {
        self.stop.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for PortalShare {
    fn drop(&mut self) {
        self.close();
    }
}

fn portal_err(e: ashpd::Error) -> AppError {
    match e {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => AppError::PermissionDenied("screen sharing was cancelled".into()),
        ashpd::Error::Response(other) => AppError::PermissionDenied(format!("the desktop refused screen sharing ({other:?})")),
        other => AppError::Other(format!("screen sharing portal: {other}")),
    }
}

/// The PipeWire side. GStreamer's `pipewiresrc` reads the node the portal granted (it handles
/// shared-memory and DMA-BUF buffers and every pixel format) and hands BGRx frames of a fixed
/// size to ENRecorder through a pipe. (The Rust PipeWire bindings need PipeWire 0.3.65+, newer
/// than Ubuntu 22.04's 0.3.48; GStreamer's element works with both.)
struct WaylandFrames {
    share: PortalShare,
    fd: Option<OwnedFd>,
    node_id: u32,
    width: u32,
    height: u32,
    child: Option<std::process::Child>,
    reader: Option<std::thread::JoinHandle<()>>,
}

/// `gst-launch-1.0` on PATH, or a clear message about the missing packages.
fn gst_launch() -> Result<std::path::PathBuf> {
    let paths = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&paths)
        .map(|d| d.join("gst-launch-1.0"))
        .find(|p| p.is_file())
        .ok_or_else(|| AppError::Unsupported("GStreamer's PipeWire support is missing. Install it with: sudo apt install gstreamer1.0-pipewire gstreamer1.0-tools".into()))
}

/// The GStreamer pipeline (fd 3 is the portal's PipeWire connection in the child).
pub fn pipeline_args(node_id: u32, width: u32, height: u32) -> Vec<String> {
    format!(
        "-q pipewiresrc fd=3 path={node_id} do-timestamp=true keepalive-time=500 always-copy=true ! videoconvert ! videoscale ! video/x-raw,format=BGRx,width={width},height={height} ! fdsink fd=1 sync=false"
    )
    .split_whitespace()
    .map(str::to_string)
    .collect()
}

impl FrameSource for WaylandFrames {
    fn start(&mut self, slot: Arc<FrameSlot>) -> Result<()> {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;
        let fd = self.fd.take().ok_or_else(|| AppError::Other("the screen share was already used".into()))?;
        let gst = gst_launch()?;
        let raw = fd.as_raw_fd();
        let mut cmd = std::process::Command::new(gst);
        cmd.args(pipeline_args(self.node_id, self.width, self.height)).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        // SAFETY: dup2 is async-signal-safe; it puts the PipeWire connection at fd 3 in the
        // child only (dup2 clears close-on-exec on the new descriptor).
        unsafe {
            cmd.pre_exec(move || if libc::dup2(raw, 3) < 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) });
        }
        let mut child = cmd.spawn().map_err(|e| AppError::Other(format!("could not start GStreamer: {e}")))?;
        drop(fd); // the child has its own copy
        let mut out = child.stdout.take().ok_or_else(|| AppError::Other("no output from GStreamer".into()))?;
        if let Some(mut err) = child.stderr.take() {
            std::thread::spawn(move || {
                let mut s = String::new();
                let _ = err.read_to_string(&mut s);
                for l in s.lines().filter(|l| !l.trim().is_empty()) {
                    log::warn!(target: "gstreamer", "{l}");
                }
            });
        }
        let (w, h) = (self.width, self.height);
        let frame = w as usize * h as usize * super::frames::BYTES_PER_PIXEL;
        self.reader = Some(
            std::thread::Builder::new()
                .name("pipewire-reader".into())
                .spawn(move || {
                    let mut buf = vec![0u8; frame];
                    while out.read_exact(&mut buf).is_ok() {
                        slot.push(&buf, w, h, w as usize * super::frames::BYTES_PER_PIXEL, PixelOrder::Bgrx, 0, 0);
                    }
                })
                .map_err(|e| AppError::Other(format!("could not start the frame reader: {e}")))?,
        );
        self.child = Some(child);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(t) = self.reader.take() {
            let _ = t.join();
        }
        self.share.close();
    }
}

impl CaptureBackend for WaylandBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::WaylandPortal
    }

    /// The portal decides what can be shared; the choice happens in its dialog.
    fn list_sources(&self) -> Result<Vec<CaptureSource>> {
        Ok(vec![CaptureSource::Portal])
    }

    fn input(&self, req: &CaptureRequest) -> Result<CaptureInput> {
        if !matches!(req.source, CaptureSource::Portal) {
            return Err(AppError::Unsupported("on Wayland the screen or window is chosen in the sharing dialog".into()));
        }
        gst_launch()?; // fail before the dialog when the reader is missing
        let (share, granted) = PortalShare::open(req.show_cursor)?;
        let (w, h) = granted.size.unwrap_or((1920, 1080));
        log::info!("screen cast granted: node {} size {:?}", granted.node_id, granted.size);
        Ok(CaptureInput {
            args: vec![],
            width: even(w),
            height: even(h),
            frames: Some(Box::new(WaylandFrames { share, fd: Some(granted.fd), node_id: granted.node_id, width: even(w), height: even(h), child: None, reader: None })),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pipeline_reads_fd_3_and_fixes_the_size() {
        let a = super::pipeline_args(42, 1280, 720).join(" ");
        assert!(a.contains("pipewiresrc fd=3 path=42"));
        assert!(a.contains("format=BGRx,width=1280,height=720"));
        assert!(a.ends_with("fdsink fd=1 sync=false"));
    }
}
