//! FFmpeg service: finds and verifies FFmpeg/FFprobe, reads the version and the encoder list,
//! and **verifies** encoders with a real test encode. An encoder only listed by
//! `ffmpeg -encoders` is a candidate; hardware encoders often appear there on machines without
//! the hardware. Only an encoder whose test encode succeeded is reported as available.

use super::process::{run, tail};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    H264,
    Hevc,
    Av1,
    Vp9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Vendor {
    Software,
    Nvidia,
    Amd,
    Intel,
    /// VA-API (Intel/AMD on Linux).
    Vaapi,
    /// Windows Media Foundation (whatever GPU the system has).
    MediaFoundation,
}

/// How a test encode must set up the hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Setup {
    Plain,
    Vaapi,
    Qsv,
}

#[derive(Debug, Clone, Copy)]
pub struct EncoderSpec {
    pub name: &'static str,
    pub codec: Codec,
    pub vendor: Vendor,
    setup: Setup,
}

const fn spec(name: &'static str, codec: Codec, vendor: Vendor, setup: Setup) -> EncoderSpec {
    EncoderSpec { name, codec, vendor, setup }
}

/// Encoders ENRecorder knows how to drive, in order of preference within each codec.
pub const CANDIDATES: &[EncoderSpec] = &[
    spec("h264_nvenc", Codec::H264, Vendor::Nvidia, Setup::Plain),
    spec("h264_amf", Codec::H264, Vendor::Amd, Setup::Plain),
    spec("h264_qsv", Codec::H264, Vendor::Intel, Setup::Qsv),
    spec("h264_vaapi", Codec::H264, Vendor::Vaapi, Setup::Vaapi),
    spec("h264_mf", Codec::H264, Vendor::MediaFoundation, Setup::Plain),
    spec("libx264", Codec::H264, Vendor::Software, Setup::Plain),
    spec("hevc_nvenc", Codec::Hevc, Vendor::Nvidia, Setup::Plain),
    spec("hevc_amf", Codec::Hevc, Vendor::Amd, Setup::Plain),
    spec("hevc_qsv", Codec::Hevc, Vendor::Intel, Setup::Qsv),
    spec("hevc_vaapi", Codec::Hevc, Vendor::Vaapi, Setup::Vaapi),
    spec("hevc_mf", Codec::Hevc, Vendor::MediaFoundation, Setup::Plain),
    spec("libx265", Codec::Hevc, Vendor::Software, Setup::Plain),
    spec("av1_nvenc", Codec::Av1, Vendor::Nvidia, Setup::Plain),
    spec("av1_amf", Codec::Av1, Vendor::Amd, Setup::Plain),
    spec("av1_qsv", Codec::Av1, Vendor::Intel, Setup::Qsv),
    spec("av1_vaapi", Codec::Av1, Vendor::Vaapi, Setup::Vaapi),
    spec("libsvtav1", Codec::Av1, Vendor::Software, Setup::Plain),
    spec("libaom-av1", Codec::Av1, Vendor::Software, Setup::Plain),
    spec("vp9_vaapi", Codec::Vp9, Vendor::Vaapi, Setup::Vaapi),
    spec("vp9_qsv", Codec::Vp9, Vendor::Intel, Setup::Qsv),
    spec("libvpx-vp9", Codec::Vp9, Vendor::Software, Setup::Plain),
];

/// A found tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub path: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderStatus {
    pub name: String,
    pub codec: Codec,
    pub vendor: Vendor,
    /// Listed by `ffmpeg -encoders`.
    pub listed: bool,
    /// A test encode succeeded.
    pub verified: bool,
    /// Why it failed (last FFmpeg lines), empty when verified or not listed.
    pub detail: String,
}

/// Result of a capability check, cached per FFmpeg path + version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub ffmpeg: ToolInfo,
    pub encoders: Vec<EncoderStatus>,
    /// Milliseconds since the epoch.
    pub checked_at: i64,
}

/// `ffmpeg version 4.4.2-0ubuntu0.22.04.1 Copyright …` → `4.4.2-0ubuntu0.22.04.1`;
/// also handles `ffmpeg version n6.1` and `ffmpeg version N-112345-g…` (git builds).
pub fn parse_version(output: &str) -> Option<String> {
    let first = output.lines().next()?;
    let mut words = first.split_whitespace();
    let tool = words.next()?;
    if !(tool == "ffmpeg" || tool == "ffprobe") || words.next()? != "version" {
        return None;
    }
    words.next().map(str::to_string)
}

/// Names of the video encoders in `ffmpeg -hide_banner -encoders` output.
pub fn parse_encoders(output: &str) -> Vec<String> {
    output
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("------"))
        .skip(1)
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let flags = it.next()?;
            let name = it.next()?;
            (flags.len() == 6 && flags.starts_with('V')).then(|| name.to_string())
        })
        .collect()
}

/// The program file names to look for on this OS.
fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).map(|d| d.join(exe(name))).find(|p| p.is_file())
}

/// Candidate locations for `name` ("ffmpeg" or "ffprobe"), best first: next to the custom
/// FFmpeg from Settings, the bundled/managed folders, then PATH.
pub fn candidates(name: &str, custom_ffmpeg: Option<&Path>, bundled_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(c) = custom_ffmpeg {
        if name == "ffmpeg" {
            out.push(c.to_path_buf());
        } else if let Some(dir) = c.parent() {
            out.push(dir.join(exe(name)));
        }
    }
    for d in bundled_dirs {
        out.push(d.join(exe(name)));
    }
    if let Some(p) = on_path(name) {
        out.push(p);
    }
    out.dedup();
    out
}

/// The first candidate that is a file and answers `-version` like the expected tool.
pub fn find_tool(name: &str, custom_ffmpeg: Option<&Path>, bundled_dirs: &[PathBuf]) -> Option<ToolInfo> {
    for p in candidates(name, custom_ffmpeg, bundled_dirs) {
        if !p.is_file() {
            continue;
        }
        match run(&p, &["-hide_banner".into(), "-version".into()], VERSION_TIMEOUT) {
            Ok(f) if f.success => {
                if let Some(v) = parse_version(&f.stdout) {
                    return Some(ToolInfo { path: p.to_string_lossy().into_owned(), version: v });
                }
                log::warn!("{} does not look like {name}", p.display());
            }
            Ok(f) => log::warn!("{} -version failed: {}", p.display(), tail(&f.stderr, 2)),
            Err(e) => log::warn!("{name} candidate {}: {e}", p.display()),
        }
    }
    None
}

/// The first VA-API render node (Linux), e.g. `/dev/dri/renderD128`.
pub fn render_node() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let mut nodes: Vec<PathBuf> = std::fs::read_dir("/dev/dri")
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("renderD")))
        .collect();
    nodes.sort();
    nodes.into_iter().next()
}

/// Arguments for a short synthetic test encode with `enc` (one second of 640×360 at 30 fps to
/// the null muxer). Hardware encoders get the device setup they need.
pub fn test_args(enc: &EncoderSpec, vaapi_device: Option<&Path>) -> Vec<String> {
    let mut a: Vec<String> = vec!["-hide_banner".into(), "-nostdin".into(), "-loglevel".into(), "error".into()];
    match enc.setup {
        Setup::Vaapi => {
            let dev = vaapi_device.map_or_else(|| "/dev/dri/renderD128".to_string(), |p| p.to_string_lossy().into_owned());
            a.extend(["-vaapi_device".into(), dev]);
        }
        Setup::Qsv => a.extend(["-init_hw_device".into(), "qsv=hw".into(), "-filter_hw_device".into(), "hw".into()]),
        Setup::Plain => {}
    }
    a.extend(["-f".into(), "lavfi".into(), "-i".into(), "testsrc2=size=640x360:rate=30".into(), "-frames:v".into(), "30".into()]);
    match enc.setup {
        Setup::Vaapi => a.extend(["-vf".into(), "format=nv12,hwupload".into()]),
        Setup::Qsv => a.extend(["-vf".into(), "format=nv12,hwupload=extra_hw_frames=64,format=qsv".into()]),
        Setup::Plain => a.extend(["-pix_fmt".into(), "yuv420p".into()]),
    }
    a.extend(["-c:v".into(), enc.name.into()]);
    // Keep the slow software AV1 encoders quick for a test.
    match enc.name {
        "libaom-av1" => a.extend(["-cpu-used".into(), "8".into(), "-row-mt".into(), "1".into()]),
        "libsvtav1" => a.extend(["-preset".into(), "12".into()]),
        "libvpx-vp9" => a.extend(["-deadline".into(), "realtime".into(), "-cpu-used".into(), "8".into()]),
        _ => {}
    }
    a.extend(["-f".into(), "null".into(), "-".into()]);
    a
}

/// Lists the encoders and test-encodes every known one that is listed. Takes a few seconds
/// (each test is bounded); run it off the UI thread.
pub fn check_capabilities(ffmpeg: &ToolInfo) -> Result<Capabilities> {
    let path = PathBuf::from(&ffmpeg.path);
    let listed_out = run(&path, &["-hide_banner".into(), "-encoders".into()], VERSION_TIMEOUT)?;
    if !listed_out.success {
        return Err(AppError::Ffmpeg(tail(&listed_out.stderr, 3)));
    }
    let listed = parse_encoders(&listed_out.stdout);
    let vaapi = render_node();
    let mut encoders = Vec::new();
    // A hardware device that hangs once (seen with QSV on GPUs without the media driver) hangs
    // for every encoder; skip its other encoders instead of waiting for each time-out.
    let mut hung: Vec<Setup> = Vec::new();
    for enc in CANDIDATES {
        let is_listed = listed.iter().any(|n| n == enc.name);
        let (verified, detail) = if !is_listed {
            (false, String::new())
        } else if enc.setup != Setup::Plain && hung.contains(&enc.setup) {
            (false, "Skipped: the hardware device did not respond to an earlier test".to_string())
        } else {
            match run(&path, &test_args(enc, vaapi.as_deref()), TEST_TIMEOUT) {
                Ok(f) if f.success => (true, String::new()),
                Ok(f) => (false, tail(&f.stderr, 2)),
                Err(e) => {
                    hung.push(enc.setup);
                    (false, e.to_string())
                }
            }
        };
        log::info!("encoder {}: listed={is_listed} verified={verified}", enc.name);
        encoders.push(EncoderStatus { name: enc.name.into(), codec: enc.codec, vendor: enc.vendor, listed: is_listed, verified, detail });
    }
    Ok(Capabilities { ffmpeg: ffmpeg.clone(), encoders, checked_at: crate::database::now_ms() })
}

/// Reads cached capabilities if they belong to this exact FFmpeg (path and version).
pub fn load_cached(file: &Path, ffmpeg: &ToolInfo) -> Option<Capabilities> {
    let text = std::fs::read_to_string(file).ok()?;
    let caps: Capabilities = serde_json::from_str(&text).ok()?;
    (caps.ffmpeg == *ffmpeg).then_some(caps)
}

pub fn save_cache(file: &Path, caps: &Capabilities) -> Result<()> {
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(caps)?)?;
    std::fs::rename(tmp, file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("ffmpeg version 4.4.2-0ubuntu0.22.04.1 Copyright (c) 2000-2021").as_deref(), Some("4.4.2-0ubuntu0.22.04.1"));
        assert_eq!(parse_version("ffprobe version n6.1 Copyright").as_deref(), Some("n6.1"));
        assert_eq!(parse_version("ffmpeg version N-112345-gabc built with gcc").as_deref(), Some("N-112345-gabc"));
        assert_eq!(parse_version("something else"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn encoder_list() {
        let out = "Encoders:\n V..... = Video\n A..... = Audio\n ------\n V....D h264_nvenc           NVIDIA NVENC\n V..... libx264              libx264 H.264\n A..... aac                  AAC\n S..... srt                  SubRip\n";
        assert_eq!(parse_encoders(out), vec!["h264_nvenc", "libx264"]);
        assert!(parse_encoders("garbage").is_empty());
    }

    #[test]
    fn test_arguments() {
        let x264 = CANDIDATES.iter().find(|e| e.name == "libx264").unwrap();
        let a = test_args(x264, None);
        assert!(a.windows(2).any(|w| w == ["-c:v", "libx264"]));
        assert!(a.windows(2).any(|w| w == ["-pix_fmt", "yuv420p"]));
        assert_eq!(a.last().map(String::as_str), Some("-"));
        assert!(!a.iter().any(|s| s.contains("vaapi")));

        let vaapi = CANDIDATES.iter().find(|e| e.name == "h264_vaapi").unwrap();
        let a = test_args(vaapi, Some(Path::new("/dev/dri/renderD129")));
        assert!(a.windows(2).any(|w| w == ["-vaapi_device", "/dev/dri/renderD129"]));
        assert!(a.windows(2).any(|w| w == ["-vf", "format=nv12,hwupload"]));

        let qsv = CANDIDATES.iter().find(|e| e.name == "hevc_qsv").unwrap();
        assert!(test_args(qsv, None).iter().any(|s| s == "qsv=hw"));
    }

    #[test]
    fn candidates_order_custom_first() {
        let c = candidates("ffprobe", Some(Path::new("/opt/ff/ffmpeg")), &[PathBuf::from("/data/tools")]);
        assert_eq!(c[0], Path::new("/opt/ff").join(exe("ffprobe")));
        assert_eq!(c[1], Path::new("/data/tools").join(exe("ffprobe")));
    }

    #[test]
    fn cache_only_for_same_ffmpeg() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("caps.json");
        let tool = ToolInfo { path: "/usr/bin/ffmpeg".into(), version: "4.4.2".into() };
        let caps = Capabilities { ffmpeg: tool.clone(), encoders: vec![], checked_at: 1 };
        save_cache(&file, &caps).unwrap();
        assert_eq!(load_cached(&file, &tool), Some(caps));
        assert_eq!(load_cached(&file, &ToolInfo { version: "6.0".into(), ..tool }), None);
    }

    /// Runs against the real FFmpeg when it is installed (skipped otherwise).
    #[test]
    fn real_ffmpeg_if_installed() {
        let Some(tool) = find_tool("ffmpeg", None, &[]) else { return };
        assert!(!tool.version.is_empty());
        let caps = check_capabilities(&tool).unwrap();
        let x264 = caps.encoders.iter().find(|e| e.name == "libx264").unwrap();
        if x264.listed {
            assert!(x264.verified, "libx264 test encode failed: {}", x264.detail);
        }
        // Something unlisted is never verified.
        assert!(caps.encoders.iter().all(|e| e.listed || !e.verified));
    }
}
