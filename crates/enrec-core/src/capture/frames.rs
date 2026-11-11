//! Frame-stream capture (Wayland PipeWire, Windows.Graphics.Capture). Those backends deliver
//! frames to ENRecorder itself; FFmpeg then reads them as raw video from a pipe.
//!
//! **Buffering and drop policy.** The backend writes into a [`FrameSlot`] that holds exactly
//! one frame: a new frame replaces the previous one (the replaced frame is counted as
//! "skipped"). A feeder thread samples the slot at the recording frame rate and writes the
//! latest frame to FFmpeg; when nothing new arrived (a still screen — PipeWire only sends
//! frames on change) it repeats the last frame, and when writing falls behind it skips ticks
//! and counts them as dropped. Memory stays at one frame plus FFmpeg's pipe buffer, however
//! long the recording runs, and the file keeps exact wall-clock timing.

use crate::error::Result;
use parking_lot::Mutex;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// FFmpeg's `bgr0` (4 bytes per pixel, B G R X), the format every frame is converted to.
pub const BYTES_PER_PIXEL: usize = 4;

/// The most recent frame, always `width × height` BGRX, tightly packed.
pub struct FrameSlot {
    pub width: u32,
    pub height: u32,
    latest: Mutex<Option<Arc<Vec<u8>>>>,
    /// Frames received from the backend.
    pub received: AtomicU64,
    /// Frames replaced before the feeder took them (more frames than the recording rate).
    pub skipped: AtomicU64,
    fresh: AtomicBool,
}

/// Pixel layout of incoming frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelOrder {
    /// B G R A/X in memory.
    Bgrx,
    /// R G B A/X in memory (swapped to BGRX).
    Rgbx,
}

impl FrameSlot {
    pub fn new(width: u32, height: u32) -> Arc<Self> {
        Arc::new(Self { width, height, latest: Mutex::new(None), received: AtomicU64::new(0), skipped: AtomicU64::new(0), fresh: AtomicBool::new(false) })
    }

    /// Stores a frame. `src_x`/`src_y` select the top-left corner to keep (area capture on
    /// backends that only deliver whole monitors). A frame of another size is cropped or
    /// padded with black, so FFmpeg always gets the size it was started with.
    #[allow(clippy::too_many_arguments)] // a frame description; a struct would only add copying
    pub fn push(&self, data: &[u8], width: u32, height: u32, stride: usize, order: PixelOrder, src_x: u32, src_y: u32) {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![0u8; w * h * BYTES_PER_PIXEL];
        let copy_w = (width.saturating_sub(src_x) as usize).min(w);
        let copy_h = (height.saturating_sub(src_y) as usize).min(h);
        for row in 0..copy_h {
            let s = (row + src_y as usize) * stride + src_x as usize * BYTES_PER_PIXEL;
            let Some(src) = data.get(s..s + copy_w * BYTES_PER_PIXEL) else { break };
            let dst = &mut out[row * w * BYTES_PER_PIXEL..][..copy_w * BYTES_PER_PIXEL];
            match order {
                PixelOrder::Bgrx => dst.copy_from_slice(src),
                PixelOrder::Rgbx => {
                    for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
                        d[0] = s[2];
                        d[1] = s[1];
                        d[2] = s[0];
                        d[3] = 255;
                    }
                }
            }
        }
        self.received.fetch_add(1, Ordering::Relaxed);
        if self.fresh.swap(true, Ordering::AcqRel) {
            self.skipped.fetch_add(1, Ordering::Relaxed);
        }
        *self.latest.lock() = Some(Arc::new(out));
    }

    /// The latest frame, if any arrived yet.
    pub fn latest(&self) -> Option<Arc<Vec<u8>>> {
        self.fresh.store(false, Ordering::Release);
        self.latest.lock().clone()
    }

    pub fn has_frame(&self) -> bool {
        self.latest.lock().is_some()
    }

    /// Waits for the first frame (a screen share can take a moment to start).
    pub fn wait_first(&self, timeout: Duration) -> bool {
        let t = Instant::now();
        while !self.has_frame() {
            if t.elapsed() > timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        true
    }
}

/// A backend that produces frames into a slot until stopped.
pub trait FrameSource: Send {
    /// Starts delivering frames into `slot` (non-blocking).
    fn start(&mut self, slot: Arc<FrameSlot>) -> Result<()>;
    /// Stops and releases the capture (idempotent).
    fn stop(&mut self);
}

/// FFmpeg input arguments for frames written to its stdin.
pub fn pipe_input_args(width: u32, height: u32, fps: u32) -> Vec<String> {
    ["-f", "rawvideo", "-pix_fmt", "bgr0", "-video_size", &format!("{width}x{height}"), "-framerate", &fps.to_string(), "-i", "pipe:0"].iter().map(|s| s.to_string()).collect()
}

/// Writes the latest frame `fps` times per second until `stop` is set or the pipe closes.
/// Returns the number of ticks dropped because writing fell behind.
pub fn feed(slot: &FrameSlot, out: &mut impl Write, fps: u32, stop: &AtomicBool, dropped: &AtomicU64) {
    let interval = Duration::from_nanos(1_000_000_000 / u64::from(fps.max(1)));
    let mut last = slot.latest().unwrap_or_else(|| Arc::new(vec![0u8; slot.width as usize * slot.height as usize * BYTES_PER_PIXEL]));
    let mut next = Instant::now();
    while !stop.load(Ordering::Acquire) {
        if let Some(f) = slot.latest() {
            last = f;
        }
        if out.write_all(&last).is_err() {
            break; // FFmpeg went away; the session notices and reports it
        }
        next += interval;
        let now = Instant::now();
        if now < next {
            std::thread::sleep(next - now);
        } else {
            let behind = ((now - next).as_nanos() / interval.as_nanos()) as u64;
            if behind > 0 {
                dropped.fetch_add(behind, Ordering::Relaxed);
                next = now;
            }
        }
    }
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_pads_and_swaps() {
        let slot = FrameSlot::new(2, 2);
        // 3×1 RGBX frame with a stride of 16 bytes: red, green, blue.
        let mut data = vec![0u8; 16];
        data[..12].copy_from_slice(&[255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 255, 0]);
        slot.push(&data, 3, 1, 16, PixelOrder::Rgbx, 1, 0);
        let f = slot.latest().unwrap();
        assert_eq!(f.len(), 2 * 2 * 4);
        // Starts at x = 1 (green), then blue; BGRX order; second row padded black.
        assert_eq!(&f[..8], &[0, 255, 0, 255, 255, 0, 0, 255]);
        assert!(f[8..].iter().all(|&b| b == 0));
    }

    #[test]
    fn counts_replaced_frames() {
        let slot = FrameSlot::new(1, 1);
        assert!(!slot.has_frame());
        slot.push(&[1, 2, 3, 4], 1, 1, 4, PixelOrder::Bgrx, 0, 0);
        slot.push(&[5, 6, 7, 8], 1, 1, 4, PixelOrder::Bgrx, 0, 0);
        assert_eq!(slot.skipped.load(Ordering::Relaxed), 1);
        assert_eq!(*slot.latest().unwrap(), vec![5, 6, 7, 8]);
        slot.push(&[9, 9, 9, 9], 1, 1, 4, PixelOrder::Bgrx, 0, 0);
        assert_eq!(slot.skipped.load(Ordering::Relaxed), 1, "taken frames are not skipped");
        assert_eq!(slot.received.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn feeder_writes_at_the_frame_rate_and_repeats_still_frames() {
        let slot = FrameSlot::new(2, 1);
        slot.push(&[1; 8], 2, 1, 8, PixelOrder::Bgrx, 0, 0);
        let stop = Arc::new(AtomicBool::new(false));
        let dropped = AtomicU64::new(0);
        let s2 = stop.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            s2.store(true, Ordering::Release);
        });
        let mut out = Vec::new();
        feed(&slot, &mut out, 30, &stop, &dropped);
        t.join().unwrap();
        let frames = out.len() / 8;
        assert_eq!(out.len() % 8, 0);
        assert!((12..=18).contains(&frames), "{frames} frames in 0.5 s at 30 fps");
        assert!(out.chunks(8).all(|c| c == [1; 8]));
    }

    #[test]
    fn pipe_arguments() {
        let a = pipe_input_args(1280, 720, 60);
        assert!(a.windows(2).any(|w| w == ["-video_size", "1280x720"]));
        assert!(a.windows(2).any(|w| w == ["-pix_fmt", "bgr0"]));
        assert_eq!(a.last().map(String::as_str), Some("pipe:0"));
    }
}
