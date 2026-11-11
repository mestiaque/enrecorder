//! Windows backend. Capture uses FFmpeg's own Windows grabbers, so the recording pipeline is the
//! same as on Linux (FFmpeg reads the screen, the session encodes and segments):
//!
//! * **`ddagrab`** (Desktop Duplication API, FFmpeg ≥ 6) for monitors and areas: GPU copy of
//!   the composed desktop, physical pixels whatever the display scaling, cursor drawn on request.
//!   Monitors are the outputs of the first graphics adapter, in DXGI order, which is exactly
//!   what `ddagrab`'s `output_idx` refers to.
//! * **`gdigrab`** (GDI) for single windows (`title=`), the whole multi-monitor desktop, and as
//!   the fallback when the FFmpeg build has no `ddagrab`.
//!
//! Windows.Graphics.Capture (per-window capture of covered windows) is not used: FFmpeg has no
//! input for it, and the frame-stream path would need a WinRT capture loop; window recording
//! therefore captures what is visible on screen, like X11.

use super::{even, BackendKind, CaptureBackend, CaptureInput, CaptureRequest, CaptureSource};
use crate::error::{AppError, Result};
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_OUTPUT_DESC};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WS_EX_TOOLWINDOW,
};

/// 0 = unknown, 1 = FFmpeg has ddagrab, 2 = it does not.
static DDAGRAB: AtomicU8 = AtomicU8::new(0);

/// Tell the backend which FFmpeg will record (checked once for the `ddagrab` filter).
pub fn set_ffmpeg(ffmpeg: &Path) {
    let has = crate::media::process::command(ffmpeg)
        .args(["-hide_banner", "-filters"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.split_whitespace().nth(1) == Some("ddagrab")))
        .unwrap_or(false);
    DDAGRAB.store(if has { 1 } else { 2 }, Ordering::Relaxed);
    log::info!("Windows capture: {}", if has { "ddagrab (Desktop Duplication)" } else { "gdigrab (GDI)" });
}

fn has_ddagrab() -> bool {
    DDAGRAB.load(Ordering::Relaxed) == 1
}

struct Output {
    idx: u32,
    name: String,
    rect: RECT,
    primary: bool,
}

/// Outputs (monitors) of adapter 0, as ddagrab numbers them.
fn outputs() -> Vec<Output> {
    let mut out = Vec::new();
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else { return out };
        let Ok(adapter) = factory.EnumAdapters1(0) else { return out };
        let mut i = 0;
        while let Ok(o) = adapter.EnumOutputs(i) {
            if let Some(d) = o.GetDesc().ok().filter(|d: &DXGI_OUTPUT_DESC| d.AttachedToDesktop.as_bool()) {
                let r = d.DesktopCoordinates;
                let name = String::from_utf16_lossy(&d.DeviceName).trim_end_matches('\0').trim_start_matches(r"\\.\").to_string();
                out.push(Output { idx: i, name, rect: r, primary: r.left == 0 && r.top == 0 });
            }
            i += 1;
        }
    }
    out
}

struct Win {
    hwnd: isize,
    title: String,
    rect: RECT,
    pid: u32,
}

unsafe extern "system" fn collect(hwnd: HWND, lp: LPARAM) -> BOOL {
    let list = &mut *(lp.0 as *mut Vec<Win>);
    if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return true.into();
    }
    if (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) & WS_EX_TOOLWINDOW.0 != 0 {
        return true.into();
    }
    // Hidden UWP/virtual-desktop windows report "cloaked".
    let mut cloaked = 0u32;
    if DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, 4).is_ok() && cloaked != 0 {
        return true.into();
    }
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return true.into();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let n = GetWindowTextW(hwnd, &mut buf);
    let title = String::from_utf16_lossy(&buf[..n as usize]);
    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() || rect.right - rect.left < 50 || rect.bottom - rect.top < 50 {
        return true.into();
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    list.push(Win { hwnd: hwnd.0 as isize, title, rect, pid });
    true.into()
}

fn windows_list() -> Vec<Win> {
    let mut list: Vec<Win> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut list as *mut _ as isize));
    }
    let own = std::process::id();
    list.retain(|w| w.pid != own && w.title != "Program Manager");
    list
}

fn exe_name(pid: u32) -> String {
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = windows::Win32::Foundation::CloseHandle(h);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

fn virtual_screen() -> RECT {
    unsafe {
        let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
        RECT { left: x, top: y, right: x + GetSystemMetrics(SM_CXVIRTUALSCREEN), bottom: y + GetSystemMetrics(SM_CYVIRTUALSCREEN) }
    }
}

pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

fn s(v: impl ToString) -> String {
    v.to_string()
}

/// `ddagrab` input: one output, optionally a part of it (offsets are relative to the output).
#[allow(clippy::too_many_arguments)]
fn ddagrab(req: &CaptureRequest, idx: u32, x: i32, y: i32, w: u32, h: u32, full: bool, origin: (i32, i32)) -> CaptureInput {
    let mut f = format!("ddagrab=output_idx={idx}:framerate={}:draw_mouse={}", req.fps, if req.show_cursor { 1 } else { 0 });
    if !full {
        f.push_str(&format!(":offset_x={x}:offset_y={y}:video_size={w}x{h}"));
    }
    // Frames arrive in GPU memory; the encoder plan expects ordinary frames.
    f.push_str(",hwdownload,format=bgra");
    CaptureInput { args: vec![s("-thread_queue_size"), s("512"), s("-f"), s("lavfi"), s("-i"), f], width: w, height: h, frames: None, origin: Some(origin) }
}

fn gdigrab(req: &CaptureRequest, target: &str, area: Option<(i32, i32, u32, u32)>) -> Vec<String> {
    let mut a = vec![s("-thread_queue_size"), s("512"), s("-f"), s("gdigrab"), s("-framerate"), req.fps.to_string(), s("-draw_mouse"), s(if req.show_cursor { "1" } else { "0" })];
    if let Some((x, y, w, h)) = area {
        a.extend([s("-offset_x"), x.to_string(), s("-offset_y"), y.to_string(), s("-video_size"), format!("{w}x{h}")]);
    }
    a.extend([s("-i"), s(target)]);
    a
}

impl CaptureBackend for WindowsBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::WindowsGraphicsCapture
    }

    fn list_sources(&self) -> Result<Vec<CaptureSource>> {
        let vs = virtual_screen();
        let mut v = vec![CaptureSource::Desktop { width: (vs.right - vs.left) as u32, height: (vs.bottom - vs.top) as u32 }];
        for o in outputs() {
            let r = o.rect;
            v.push(CaptureSource::Monitor { id: o.idx.to_string(), name: o.name, width: (r.right - r.left) as u32, height: (r.bottom - r.top) as u32, x: r.left, y: r.top, primary: o.primary });
        }
        for w in windows_list() {
            let r = w.rect;
            v.push(CaptureSource::Window { id: format!("{:#x}", w.hwnd), title: w.title, app: exe_name(w.pid), width: (r.right - r.left) as u32, height: (r.bottom - r.top) as u32 });
        }
        Ok(v)
    }

    fn input(&self, req: &CaptureRequest) -> Result<CaptureInput> {
        let outs = outputs();
        match &req.source {
            CaptureSource::Desktop { .. } => {
                if has_ddagrab() && outs.len() == 1 {
                    let r = outs[0].rect;
                    return Ok(ddagrab(req, outs[0].idx, 0, 0, even((r.right - r.left) as u32), even((r.bottom - r.top) as u32), true, (r.left, r.top)));
                }
                let vs = virtual_screen();
                Ok(CaptureInput { args: gdigrab(req, "desktop", None), width: (vs.right - vs.left) as u32, height: (vs.bottom - vs.top) as u32, frames: None, origin: Some((vs.left, vs.top)) })
            }
            CaptureSource::Monitor { id, .. } => {
                let o = outs.iter().find(|o| o.idx.to_string() == *id).ok_or_else(|| AppError::Other("That monitor is no longer connected".into()))?;
                let (w, h) = (even((o.rect.right - o.rect.left) as u32), even((o.rect.bottom - o.rect.top) as u32));
                if has_ddagrab() {
                    return Ok(ddagrab(req, o.idx, 0, 0, w, h, true, (o.rect.left, o.rect.top)));
                }
                Ok(CaptureInput { args: gdigrab(req, "desktop", Some((o.rect.left, o.rect.top, w, h))), width: w, height: h, frames: None, origin: Some((o.rect.left, o.rect.top)) })
            }
            CaptureSource::Region { x, y, width, height } => {
                let (w, h) = (even(*width), even(*height));
                if has_ddagrab() {
                    // ddagrab captures one output: the area must lie on a single monitor.
                    if let Some(o) = outs.iter().find(|o| *x >= o.rect.left && *y >= o.rect.top && x + w as i32 <= o.rect.right && y + h as i32 <= o.rect.bottom) {
                        return Ok(ddagrab(req, o.idx, x - o.rect.left, y - o.rect.top, w, h, false, (*x, *y)));
                    }
                }
                Ok(CaptureInput { args: gdigrab(req, "desktop", Some((*x, *y, w, h))), width: w, height: h, frames: None, origin: Some((*x, *y)) })
            }
            CaptureSource::Window { id, .. } => {
                let w = windows_list().into_iter().find(|w| format!("{:#x}", w.hwnd) == *id).ok_or_else(|| AppError::Other("That window was closed".into()))?;
                let (ww, wh) = (even((w.rect.right - w.rect.left) as u32), even((w.rect.bottom - w.rect.top) as u32));
                Ok(CaptureInput { args: gdigrab(req, &format!("title={}", w.title), None), width: ww, height: wh, frames: None, origin: Some((w.rect.left, w.rect.top)) })
            }
            CaptureSource::Portal => Err(AppError::Unsupported("the sharing dialog is only used on Wayland".into())),
        }
    }
}
