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
    /// "rectangle" | "rounded" | "circle"
    pub shape: String,
    /// A frame around the camera picture: "" (none) or a colour "#rrggbb".
    pub border: String,
    /// A soft dark shadow under the camera picture.
    pub shadow: bool,
}

impl Default for WebcamOptions {
    fn default() -> Self {
        Self { device: None, position: "bottom-right".into(), size: 22, shape: "circle".into(), border: String::new(), shadow: false }
    }
}

/// "#rrggbb" → FFmpeg colour "0xRRGGBB" (None for anything else).
fn ffmpeg_colour(c: &str) -> Option<String> {
    let h = c.strip_prefix('#')?;
    (h.len() == 6 && h.chars().all(|x| x.is_ascii_hexdigit())).then(|| format!("0x{}", h.to_ascii_uppercase()))
}

/// Alpha mask expression for a shape of size W×H (geq): circle, rounded rectangle or none.
fn mask(shape: &str) -> Option<&'static str> {
    match shape {
        "circle" => Some("if(lte(hypot(X-W/2,Y-H/2),W/2-1),255,0)"),
        // Corner radius 12 % of the width: outside the corner circles is transparent.
        "rounded" => Some("if(gt(abs(X-W/2),W/2-W*0.12)*gt(abs(Y-H/2),H/2-W*0.12)*gt(hypot(abs(X-W/2)-(W/2-W*0.12),abs(Y-H/2)-(H/2-W*0.12)),W*0.12),0,255)"),
        _ => None,
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
    let shape = match o.shape.as_str() {
        "circle" | "rounded" => o.shape.as_str(),
        _ => "rectangle",
    };
    // The camera picture (square for a circle), masked to its shape.
    let mut chain = if shape == "circle" { format!("[{cam_input}:v]crop='min(iw,ih)':'min(iw,ih)',scale={cw}:{cw}") } else { format!("[{cam_input}:v]scale={cw}:-2,crop=trunc(iw/2)*2:trunc(ih/2)*2") };
    if let Some(m) = mask(shape) {
        chain.push_str(&format!(",format=yuva420p,geq=lum='p(X,Y)':cb='p(X,Y)':cr='p(X,Y)':a='{m}'"));
    }
    let mut graph = vec![format!("[0:v]{base}[base]")];
    let border = ffmpeg_colour(&o.border);
    let b = (cw / 40).max(3) & !1;
    let mut cam = "cam".to_string();
    graph.push(format!("{chain}[{cam}]"));
    // Frame: the same shape, `b` larger on every side, in the border colour, under the picture.
    if let Some(colour) = border {
        let src = format!("[{cam}]");
        let size = if shape == "circle" { format!("{s}x{s}", s = cw + 2 * b) } else { "PAD".into() };
        if shape == "circle" {
            graph.push(format!("color=c={colour}:s={size},format=yuva420p,geq=lum='p(X,Y)':cb='p(X,Y)':cr='p(X,Y)':a='{}'[ring]", mask("circle").unwrap_or("255")));
            graph.push(format!("[ring]{src}overlay={b}:{b}:shortest=1[framed]"));
        } else {
            // pad keeps transparency outside the rounded corners of the frame too.
            let fill = mask(shape).map_or(String::new(), |m| format!(",format=yuva420p,geq=lum='p(X,Y)':cb='p(X,Y)':cr='p(X,Y)':a='if({m},255,0)'"));
            graph.push(format!("{src}split[c1][c2];[c1]pad=iw+{d}:ih+{d}:{b}:{b}:color={colour},drawbox=0:0:iw:ih:color={colour}:t=fill{fill}[frame];[frame][c2]overlay={b}:{b}[framed]", d = 2 * b));
        }
        cam = "framed".into();
    }
    // Shadow: a dark, half-transparent copy of the shape a little lower and to the right.
    if o.shadow {
        let off = (cw / 50).max(4);
        graph.push(format!("[{cam}]split[s1][s2];[s1]format=yuva420p,lutyuv=y=16:u=128:v=128:a='val*0.45'[sh];[sh]pad=iw+{off}:ih+{off}:{off}:{off}:color=black@0[shp];[shp][s2]overlay=0:0[shadowed]"));
        cam = "shadowed".into();
    }
    graph.push(format!("[base][{cam}]overlay=x={x}:y={y}:eof_action=pass,{after}[vout]"));
    graph.join(";")
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
        let g = overlay_graph(&WebcamOptions { device: Some("/dev/video0".into()), position: "top-left".into(), size: 25, shape: "rectangle".into(), ..Default::default() }, 1, 1920, "scale=1920:1080", "format=yuv420p");
        assert!(g.starts_with("[0:v]scale=1920:1080[base];[1:v]scale=480:-2") && g.contains("[base][cam]overlay=x=32:y=32") && g.ends_with("format=yuv420p[vout]"), "{g}");
        let styled = overlay_graph(&WebcamOptions { device: Some("x".into()), shape: "rounded".into(), border: "#ffffff".into(), shadow: true, ..Default::default() }, 1, 1280, "null", "format=yuv420p");
        assert!(styled.contains("color=0xFFFFFF") && styled.contains("[shadowed]") && styled.contains("[base][shadowed]overlay"), "{styled}");
        let ring = overlay_graph(&WebcamOptions { device: Some("x".into()), border: "#ff0000".into(), ..Default::default() }, 1, 1280, "null", "format=yuv420p");
        assert!(ring.contains("color=c=0xFF0000:s=") && ring.contains("[base][framed]overlay"), "{ring}");
        assert_eq!(ffmpeg_colour("blue"), None);
        let c = overlay_graph(&WebcamOptions::default(), 1, 1280, "crop=iw:ih", "format=nv12,hwupload");
        assert!(c.contains("geq=") && c.contains("x=W-w-21:y=H-h-21"), "{c}");
    }

    /// Every shape/border/shadow combination runs in the real FFmpeg (a test pattern stands in
    /// for the camera).
    #[test]
    fn graphs_run_in_ffmpeg() {
        let Some(ff) = crate::media::ffmpeg::find_tool("ffmpeg", None, &[]) else { return };
        for shape in ["circle", "rounded", "rectangle"] {
            for (border, shadow) in [("", false), ("#ffffff", true), ("#ff0000", false)] {
                let o = WebcamOptions { device: Some("x".into()), shape: shape.into(), border: border.into(), shadow, ..Default::default() };
                let g = overlay_graph(&o, 1, 640, "null", "format=yuv420p");
                let args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=10", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=10", "-filter_complex", &g, "-map", "[vout]", "-frames:v", "5", "-f", "null", "-"]
                    .iter()
                    .map(|x| x.to_string())
                    .collect();
                let out = crate::media::process::run(std::path::Path::new(&ff.path), &args, std::time::Duration::from_secs(30)).unwrap();
                assert!(out.success, "{shape} border={border} shadow={shadow}: {}\n{g}", out.stderr);
            }
        }
    }
}
