//! Webcam overlay (phase 6): camera devices and the FFmpeg input + overlay filter that puts
//! the camera picture into a corner of the screen recording (rectangle or circle).

use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    /// What FFmpeg opens: /dev/videoN (Linux) or the DirectShow name (Windows).
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebcamOptions {
    pub device: Option<String>,
    /// "bottom-right" | "bottom-left" | "top-right" | "top-left"
    pub position: String,
    /// Width of the camera picture as a percentage of the recording width (10–40).
    pub size: u32,
    /// "rectangle" | "circle"
    pub shape: String,
}

impl Default for WebcamOptions {
    fn default() -> Self {
        Self { device: None, position: "bottom-right".into(), size: 22, shape: "circle".into() }
    }
}

/// Video capture devices (Linux: from sysfs, only the first node of each camera).
pub fn list_cameras(ffmpeg: Option<&std::path::Path>) -> Result<Vec<Camera>> {
    if cfg!(windows) {
        let Some(ffmpeg) = ffmpeg else { return Ok(vec![]) };
        let args: Vec<String> = ["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"].iter().map(|s| s.to_string()).collect();
        let out = crate::media::process::run(ffmpeg, &args, std::time::Duration::from_secs(10))?;
        return Ok(parse_dshow_video(&out.stderr));
    }
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir("/sys/class/video4linux") else { return Ok(out) };
    let mut nodes: Vec<_> = dir.flatten().map(|e| e.path()).collect();
    nodes.sort();
    for n in nodes {
        // index 0 = the picture stream; other nodes of the same camera carry metadata.
        let index = std::fs::read_to_string(n.join("index")).unwrap_or_else(|_| "0".into());
        if index.trim() != "0" {
            continue;
        }
        let name = std::fs::read_to_string(n.join("name")).unwrap_or_default().trim().to_string();
        let dev = format!("/dev/{}", n.file_name().unwrap_or_default().to_string_lossy());
        out.push(Camera { name: if name.is_empty() { dev.clone() } else { name }, id: dev });
    }
    Ok(out)
}

pub fn parse_dshow_video(stderr: &str) -> Vec<Camera> {
    stderr
        .lines()
        .filter_map(|l| {
            let start = l.find('"')?;
            let rest = &l[start + 1..];
            let end = rest.find('"')?;
            rest[end..].contains("(video)").then(|| Camera { id: rest[..end].to_string(), name: rest[..end].to_string() })
        })
        .collect()
}

fn s(v: &str) -> String {
    v.to_string()
}

pub fn input_args(device: &str, fps: u32) -> Vec<String> {
    if cfg!(windows) {
        vec![s("-thread_queue_size"), s("512"), s("-f"), s("dshow"), s("-rtbufsize"), s("64M"), s("-i"), format!("video={device}")]
    } else {
        vec![s("-thread_queue_size"), s("512"), s("-f"), s("v4l2"), s("-framerate"), fps.min(30).to_string(), s("-i"), device.to_string()]
    }
}

/// Filter graph: `[0:v]` → `base` filters, camera `[cam:v]` scaled (and masked), overlaid in the
/// corner, then `after` (pixel format / GPU upload) → `[vout]`.
pub fn overlay_graph(o: &WebcamOptions, cam_input: usize, out_w: u32, base: &str, after: &str) -> String {
    let cw = ((out_w as f64 * o.size.clamp(10, 40) as f64 / 100.0) as u32) & !1;
    let margin = (out_w / 60).max(8);
    let (x, y) = match o.position.as_str() {
        "top-left" => (format!("{margin}"), format!("{margin}")),
        "top-right" => (format!("W-w-{margin}"), format!("{margin}")),
        "bottom-left" => (format!("{margin}"), format!("H-h-{margin}")),
        _ => (format!("W-w-{margin}"), format!("H-h-{margin}")),
    };
    let cam = if o.shape == "circle" {
        format!("[{cam_input}:v]crop='min(iw,ih)':'min(iw,ih)',scale={cw}:{cw},format=yuva420p,geq=lum='p(X,Y)':cb='p(X,Y)':cr='p(X,Y)':a='if(lte(hypot(X-W/2,Y-H/2),W/2-1),255,0)'[cam]")
    } else {
        format!("[{cam_input}:v]scale={cw}:-2[cam]")
    };
    format!("[0:v]{base}[base];{cam};[base][cam]overlay=x={x}:y={y}:eof_action=pass,{after}[vout]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dshow_cameras() {
        let err = "[dshow @ 1] \"Integrated Camera\" (video)\n[dshow @ 1] \"Microphone (Realtek)\" (audio)\n";
        assert_eq!(parse_dshow_video(err), vec![Camera { id: "Integrated Camera".into(), name: "Integrated Camera".into() }]);
    }

    #[test]
    fn graphs() {
        let g = overlay_graph(&WebcamOptions { device: Some("/dev/video0".into()), position: "top-left".into(), size: 25, shape: "rectangle".into() }, 1, 1920, "scale=1920:1080", "format=yuv420p");
        assert!(g.starts_with("[0:v]scale=1920:1080[base];[1:v]scale=480:-2[cam]") && g.contains("overlay=x=32:y=32") && g.ends_with("format=yuv420p[vout]"), "{g}");
        let c = overlay_graph(&WebcamOptions::default(), 1, 1280, "crop=iw:ih", "format=nv12,hwupload");
        assert!(c.contains("geq=") && c.contains("x=W-w-21:y=H-h-21"), "{c}");
    }
}
