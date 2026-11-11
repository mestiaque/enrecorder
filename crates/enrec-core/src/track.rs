//! Pointer and keyboard tracks for **post-processing** effects.
//!
//! While recording (when the user turned it on), ENRecorder samples where the pointer is, which
//! buttons are down and — only if separately enabled — which keys are pressed, and saves that as
//! a small JSON *track* in its data folder (never next to the media, never uploaded). Nothing is
//! drawn into the recording itself. When exporting, the track becomes an ASS subtitle overlay
//! (pointer highlight, click ripples, keystroke captions) burnt into the new file. The original
//! recording stays untouched — this is what the brief means by keeping captured cursor effects
//! and post-processing effects apart.
//!
//! Sampling uses the display server's query calls (X11 `QueryPointer`/`QueryKeymap`, Windows
//! `GetCursorPos`/`GetAsyncKeyState`), not an input hook. Wayland does not let applications see
//! the pointer or keyboard outside their own windows, so tracks are not available there.

use crate::error::{AppError, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEvent {
    /// Recorded time in ms (pauses excluded), from the start of the file.
    pub t: u64,
    /// Pointer position in the recorded picture's source pixels (before scaling).
    pub x: i32,
    pub y: i32,
    /// "left" | "right" | "middle" when a button went down at this moment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click: Option<String>,
    /// A key combination pressed at this moment, e.g. "Ctrl+S".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub version: u32,
    /// Size of the captured area in source pixels (the coordinates' space).
    pub width: u32,
    pub height: u32,
    pub events: Vec<TrackEvent>,
}

impl Track {
    pub fn has_clicks(&self) -> bool {
        self.events.iter().any(|e| e.click.is_some())
    }

    pub fn has_keys(&self) -> bool {
        self.events.iter().any(|e| e.key.is_some())
    }

    /// The events in `[from, to)` ms, shifted to start at 0 (one file of a split recording).
    pub fn slice(&self, from: u64, to: u64) -> Track {
        Track { version: self.version, width: self.width, height: self.height, events: self.events.iter().filter(|e| e.t >= from && e.t < to).map(|e| TrackEvent { t: e.t - from, ..e.clone() }).collect() }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, serde_json::to_vec(self)?)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Track> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }
}

/// One sample of the input state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputState {
    /// Pointer position in desktop pixels.
    pub x: i32,
    pub y: i32,
    /// Buttons down: bit 0 left, bit 1 middle, bit 2 right.
    pub buttons: u8,
    /// Keys down, as display names ("A", "Ctrl", "Enter" …).
    pub keys: Vec<String>,
}

/// Reads the input state (one implementation per display server).
pub trait InputSampler: Send {
    fn sample(&mut self, keys: bool) -> Result<InputState>;
}

const MODIFIERS: &[&str] = &["Ctrl", "Shift", "Alt", "Super"];

/// Builds a combination label from newly pressed keys and the modifiers held: "Ctrl+Shift+S".
/// A modifier alone is not a keystroke worth showing; plain letters are shown as typed.
pub fn combo(newly: &[String], held: &[String]) -> Option<String> {
    let key = newly.iter().find(|k| !MODIFIERS.contains(&k.as_str()))?;
    let mut parts: Vec<&str> = MODIFIERS.iter().copied().filter(|m| held.iter().any(|h| h == m)).collect();
    parts.push(key);
    Some(parts.join("+"))
}

/// Samples the input about 60 times a second on its own thread and records events in recorded time.
pub struct Tracker {
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    events: Arc<Mutex<Vec<TrackEvent>>>,
    width: u32,
    height: u32,
}

impl Tracker {
    /// `origin` is the top-left of the captured area on the desktop; positions outside the area
    /// are still recorded (clamped when drawn) so the highlight can follow the pointer to the edge.
    pub fn start(mut sampler: Box<dyn InputSampler>, origin: (i32, i32), size: (u32, u32), keys: bool) -> Tracker {
        let stop = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Vec::new()));
        let (st, pa, ev) = (stop.clone(), paused.clone(), events.clone());
        let thread = std::thread::Builder::new()
            .name("input-track".into())
            .spawn(move || {
                // Recorded clock: wall time minus pauses.
                let mut clock = Duration::ZERO;
                let mut last = Instant::now();
                let mut prev = InputState::default();
                let mut last_pos = (i32::MIN, i32::MIN);
                let mut errors = 0;
                // 16 ms: a quick click is held for ~40–100 ms, so polling this often sees it.
                while !st.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(16));
                    let now = Instant::now();
                    if pa.load(Ordering::Acquire) {
                        last = now;
                        continue;
                    }
                    clock += now - last;
                    last = now;
                    let s = match sampler.sample(keys) {
                        Ok(s) => s,
                        Err(e) => {
                            errors += 1;
                            if errors == 1 {
                                log::warn!("input track: {e}");
                            }
                            continue;
                        }
                    };
                    let t = clock.as_millis() as u64;
                    let (x, y) = (s.x - origin.0, s.y - origin.1);
                    let pressed = s.buttons & !prev.buttons;
                    let click = [(1u8, "left"), (2, "middle"), (4, "right")].iter().find(|(b, _)| pressed & b != 0).map(|(_, n)| n.to_string());
                    let newly: Vec<String> = s.keys.iter().filter(|k| !prev.keys.contains(k)).cloned().collect();
                    let key = if keys { combo(&newly, &s.keys) } else { None };
                    if click.is_some() || key.is_some() || (x, y) != last_pos {
                        ev.lock().push(TrackEvent { t, x, y, click, key });
                        last_pos = (x, y);
                    }
                    prev = s;
                }
            })
            .ok();
        Tracker { stop, paused, thread, events, width: size.0, height: size.1 }
    }

    pub fn pause(&self, on: bool) {
        self.paused.store(on, Ordering::Release);
    }

    /// Stops sampling and returns the whole track.
    pub fn finish(&mut self) -> Track {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        Track { version: 1, width: self.width, height: self.height, events: std::mem::take(&mut *self.events.lock()) }
    }

    /// The track so far (file splitting saves each file's part while recording goes on).
    pub fn snapshot(&self) -> Track {
        Track { version: 1, width: self.width, height: self.height, events: self.events.lock().clone() }
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ------------------------------------------------------------------ samplers

#[cfg(target_os = "linux")]
pub mod x11 {
    use super::{InputSampler, InputState};
    use crate::error::{AppError, Result};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt as _, KeyButMask};
    use x11rb::rust_connection::RustConnection;

    /// X11: `QueryPointer` and `QueryKeymap` on the root window (no grab, no hook).
    pub struct X11Sampler {
        conn: RustConnection,
        root: u32,
        /// Keycode → display name (from the keyboard mapping, read once).
        names: Vec<Option<String>>,
    }

    fn x_err(e: impl std::fmt::Display) -> AppError {
        AppError::Other(format!("X11: {e}"))
    }

    /// A readable name for an X keysym (letters upper-case, common keys spelled out).
    pub fn keysym_name(sym: u32) -> Option<String> {
        Some(match sym {
            0x20 => "Space".into(),
            0x21..=0x7e => (sym as u8 as char).to_ascii_uppercase().to_string(),
            0xff08 => "Backspace".into(),
            0xff09 => "Tab".into(),
            0xff0d => "Enter".into(),
            0xff1b => "Esc".into(),
            0xffff => "Delete".into(),
            0xff50 => "Home".into(),
            0xff51 => "Left".into(),
            0xff52 => "Up".into(),
            0xff53 => "Right".into(),
            0xff54 => "Down".into(),
            0xff55 => "PageUp".into(),
            0xff56 => "PageDown".into(),
            0xff57 => "End".into(),
            0xff63 => "Insert".into(),
            0xffbe..=0xffc9 => format!("F{}", sym - 0xffbe + 1),
            0xffe1 | 0xffe2 => "Shift".into(),
            0xffe3 | 0xffe4 => "Ctrl".into(),
            0xffe9 | 0xffea => "Alt".into(),
            0xffeb | 0xffec => "Super".into(),
            _ => return None,
        })
    }

    impl X11Sampler {
        pub fn new() -> Result<Self> {
            let (conn, screen) = x11rb::connect(None).map_err(|e| AppError::Unsupported(format!("cannot connect to the X server: {e}")))?;
            let root = conn.setup().roots[screen].root;
            let (min, max) = (conn.setup().min_keycode, conn.setup().max_keycode);
            let map = conn.get_keyboard_mapping(min, max - min + 1).map_err(x_err)?.reply().map_err(x_err)?;
            let per = map.keysyms_per_keycode.max(1) as usize;
            let mut names = vec![None; 256];
            for (i, chunk) in map.keysyms.chunks(per).enumerate() {
                if let Some(sym) = chunk.first() {
                    names[min as usize + i] = keysym_name(*sym);
                }
            }
            Ok(Self { conn, root, names })
        }
    }

    impl InputSampler for X11Sampler {
        fn sample(&mut self, keys: bool) -> Result<InputState> {
            let p = self.conn.query_pointer(self.root).map_err(x_err)?.reply().map_err(x_err)?;
            let m = p.mask;
            let mut buttons = 0u8;
            if m.contains(KeyButMask::BUTTON1) {
                buttons |= 1;
            }
            if m.contains(KeyButMask::BUTTON2) {
                buttons |= 2;
            }
            if m.contains(KeyButMask::BUTTON3) {
                buttons |= 4;
            }
            let mut down = Vec::new();
            if keys {
                let km = self.conn.query_keymap().map_err(x_err)?.reply().map_err(x_err)?;
                for (byte, bits) in km.keys.iter().enumerate() {
                    for bit in 0..8 {
                        if bits & (1 << bit) != 0 {
                            if let Some(Some(n)) = self.names.get(byte * 8 + bit) {
                                if !down.contains(n) {
                                    down.push(n.clone());
                                }
                            }
                        }
                    }
                }
            }
            Ok(InputState { x: p.root_x as i32, y: p.root_y as i32, buttons, keys: down })
        }
    }
}

#[cfg(windows)]
pub mod win {
    use super::{InputSampler, InputState};
    use crate::error::Result;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    /// Windows: `GetCursorPos` and `GetAsyncKeyState` (polling, no hook).
    pub struct WinSampler;

    fn down(vk: i32) -> bool {
        // SAFETY: plain Win32 call with a virtual-key code.
        (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
    }

    fn vk_name(vk: i32) -> Option<String> {
        Some(match vk {
            0x08 => "Backspace".into(),
            0x09 => "Tab".into(),
            0x0D => "Enter".into(),
            0x1B => "Esc".into(),
            0x20 => "Space".into(),
            0x21 => "PageUp".into(),
            0x22 => "PageDown".into(),
            0x23 => "End".into(),
            0x24 => "Home".into(),
            0x25 => "Left".into(),
            0x26 => "Up".into(),
            0x27 => "Right".into(),
            0x28 => "Down".into(),
            0x2D => "Insert".into(),
            0x2E => "Delete".into(),
            0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
            0x70..=0x7B => format!("F{}", vk - 0x6F),
            0x10 => "Shift".into(),
            0x11 => "Ctrl".into(),
            0x12 => "Alt".into(),
            0x5B => "Super".into(),
            _ => return None,
        })
    }

    impl InputSampler for WinSampler {
        fn sample(&mut self, keys: bool) -> Result<InputState> {
            let mut p = POINT::default();
            // SAFETY: GetCursorPos writes into the POINT we own.
            let _ = unsafe { GetCursorPos(&mut p) };
            let mut buttons = 0u8;
            for (vk, bit) in [(0x01, 1u8), (0x04, 2), (0x02, 4)] {
                if down(vk) {
                    buttons |= bit;
                }
            }
            let mut list = Vec::new();
            if keys {
                for vk in (0x08..=0x7B).chain([0x5B]) {
                    if let Some(n) = vk_name(vk).filter(|_| down(vk)) {
                        if !list.contains(&n) {
                            list.push(n);
                        }
                    }
                }
            }
            Ok(InputState { x: p.x, y: p.y, buttons, keys: list })
        }
    }
}

/// The sampler for this session's display server, when one exists.
pub fn sampler_for(display: crate::system::DisplayServer) -> Result<Box<dyn InputSampler>> {
    use crate::system::DisplayServer;
    match display {
        #[cfg(target_os = "linux")]
        DisplayServer::X11 => Ok(Box::new(x11::X11Sampler::new()?)),
        #[cfg(windows)]
        DisplayServer::Windows => Ok(Box::new(win::WinSampler)),
        _ => Err(AppError::Unsupported("pointer and key tracks need X11 or Windows (Wayland does not allow them)".into())),
    }
}

// ------------------------------------------------------------------ overlay

/// ASS time "h:mm:ss.cc".
fn ass_time(ms: u64) -> String {
    let cs = ms / 10;
    format!("{}:{:02}:{:02}.{:02}", cs / 360_000, (cs / 6000) % 60, (cs / 100) % 60, cs % 100)
}

/// A circle of radius `r` as an ASS drawing (centred on its position).
fn circle(r: i32) -> String {
    let k = (r as f64 * 0.5523).round() as i32;
    format!("m 0 {n} b {k} {n} {r} {nk} {r} 0 b {r} {k} {k} {r} 0 {r} b {nk2} {r} {n2} {k} {n2} 0 b {n2} {nk} {nk2} {n} 0 {n}", n = -r, nk = -k, r = r, k = k, nk2 = -k, n2 = -r)
}

/// Builds an ASS overlay for a video of `out_w`×`out_h`: a soft highlight that follows the
/// pointer, an expanding ring on each click, and key combinations at the bottom.
/// `offset_ms` and `speed` map track time to output time (trimmed / sped-up exports).
#[allow(clippy::too_many_arguments)] // the output mapping is one set of numbers
pub fn to_ass(track: &Track, out_w: u32, out_h: u32, highlight: bool, keys: bool, offset_ms: u64, speed: f64, until_ms: Option<u64>) -> String {
    let sx = out_w as f64 / track.width.max(1) as f64;
    let sy = out_h as f64 / track.height.max(1) as f64;
    let r = ((out_h as f64) * 0.025).clamp(10.0, 40.0) as i32;
    let font = ((out_h as f64) * 0.045).clamp(16.0, 64.0) as i32;
    let mut out = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {out_w}\nPlayResY: {out_h}\nScaledBorderAndShadow: yes\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Shape,Arial,20,&H0000D7FF,&H00000000,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,0,0,7,0,0,0,1\nStyle: Keys,DejaVu Sans,{font},&H00FFFFFF,&H00000000,&H00000000,&H96000000,1,0,0,0,100,100,0,0,3,{pad},0,2,20,20,{mv},1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        pad = font / 4,
        mv = out_h / 12
    );
    let map_t = |t: u64| -> Option<u64> {
        if t < offset_ms || until_ms.is_some_and(|u| t >= u) {
            return None;
        }
        Some(((t - offset_ms) as f64 / speed.max(0.01)) as u64)
    };
    let pos = |e: &TrackEvent| (((e.x.max(0) as f64) * sx).round().min(out_w as f64) as i32, ((e.y.max(0) as f64) * sy).round().min(out_h as f64) as i32);
    if highlight {
        let evs: Vec<&TrackEvent> = track.events.iter().filter(|e| map_t(e.t).is_some()).collect();
        for (i, e) in evs.iter().enumerate() {
            let start = map_t(e.t).unwrap_or(0);
            let end = evs.get(i + 1).and_then(|n| map_t(n.t)).unwrap_or(start + 1000).max(start + 20);
            let (x, y) = pos(e);
            out.push_str(&format!("Dialogue: 0,{},{},Shape,,0,0,0,,{{\\an7\\pos({x},{y})\\p1\\1c&H00D7FF&\\1a&HA0&\\bord0\\shad0}}{}{{\\p0}}\n", ass_time(start), ass_time(end), circle(r)));
            if let Some(c) = &e.click {
                let colour = if c == "right" { "&H6B6BFF&" } else { "&H00A5FF&" };
                out.push_str(&format!(
                    "Dialogue: 1,{},{},Shape,,0,0,0,,{{\\an7\\pos({x},{y})\\p1\\1a&HFF&\\3c{colour}\\bord4\\shad0\\fscx60\\fscy60\\t(0,400,\\fscx220\\fscy220\\3a&HFF&)}}{}{{\\p0}}\n",
                    ass_time(start),
                    ass_time(start + 450),
                    circle(r)
                ));
            }
        }
    }
    if keys {
        let ks: Vec<(u64, &str)> = track.events.iter().filter_map(|e| Some((map_t(e.t)?, e.key.as_deref()?))).collect();
        for (i, (t, k)) in ks.iter().enumerate() {
            // Shown for 1.2 s, or until the next keystroke.
            let end = ks.get(i + 1).map(|n| n.0).unwrap_or(t + 1200).min(t + 1200).max(t + 150);
            let text = k.replace('{', "(").replace('}', ")");
            out.push_str(&format!("Dialogue: 2,{},{},Keys,,0,0,0,,{text}\n", ass_time(*t), ass_time(end)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Track {
        Track {
            version: 1,
            width: 1000,
            height: 500,
            events: vec![
                TrackEvent { t: 0, x: 100, y: 100, click: None, key: None },
                TrackEvent { t: 500, x: 500, y: 250, click: Some("left".into()), key: None },
                TrackEvent { t: 900, x: 500, y: 250, click: None, key: Some("Ctrl+S".into()) },
                TrackEvent { t: 3000, x: 900, y: 400, click: Some("right".into()), key: None },
            ],
        }
    }

    #[test]
    fn combos() {
        let held = vec!["Ctrl".to_string(), "Shift".into(), "S".into()];
        assert_eq!(combo(&["S".into()], &held).as_deref(), Some("Ctrl+Shift+S"));
        assert_eq!(combo(&["Ctrl".into()], &["Ctrl".into()]), None, "a modifier alone is not shown");
        assert_eq!(combo(&["A".into()], &["A".into()]).as_deref(), Some("A"));
    }

    #[test]
    fn times() {
        assert_eq!(ass_time(0), "0:00:00.00");
        assert_eq!(ass_time(3_723_450), "1:02:03.45");
    }

    #[test]
    fn overlay_scales_and_maps_time() {
        let ass = to_ass(&track(), 500, 250, true, true, 0, 1.0, None);
        assert!(ass.contains("PlayResX: 500"));
        // (500, 250) in a 1000×500 source → (250, 125) in a 500×250 output.
        assert!(ass.contains("\\pos(250,125)"));
        assert_eq!(ass.matches("\\t(0,400").count(), 2, "one ripple per click");
        assert!(ass.contains(",Keys,,0,0,0,,Ctrl+S"));
        // Trimmed from 0.8 s and played twice as fast: the right click at 3.0 s lands at 1.1 s.
        let cut = to_ass(&track(), 1000, 500, true, false, 800, 2.0, None);
        assert!(cut.contains("Dialogue: 1,0:00:01.10"), "{cut}");
        assert!(!cut.contains("Keys,,"));
        // Events after the end of a trimmed range are left out.
        assert_eq!(to_ass(&track(), 1000, 500, true, false, 0, 1.0, Some(1000)).matches("\\t(0,400").count(), 1);
    }

    #[test]
    fn slices_and_roundtrips() {
        let t = track().slice(500, 3000);
        assert_eq!(t.events.len(), 2);
        assert_eq!(t.events[0].t, 0);
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.json");
        t.save(&p).unwrap();
        assert_eq!(Track::load(&p).unwrap(), t);
        assert!(t.has_clicks() && t.has_keys());
    }

    /// The real X server (skipped without one): sampling works and positions are on screen.
    #[cfg(target_os = "linux")]
    #[test]
    fn x11_sampler_if_available() {
        let Ok(mut s) = x11::X11Sampler::new() else { return };
        let st = s.sample(true).unwrap();
        assert!(st.x >= 0 && st.y >= 0);
        assert_eq!(x11::keysym_name(0x61).as_deref(), Some("A"));
        assert_eq!(x11::keysym_name(0xffe3).as_deref(), Some("Ctrl"));
    }
}
