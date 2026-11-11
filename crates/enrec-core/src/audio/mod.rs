//! Audio: input devices (microphones and system audio), the FFmpeg inputs that read them, and
//! the mixing/track layout for recordings.
//!
//! Capture goes through FFmpeg like video, so audio and video share one process, one clock and
//! one segment file per pause (that is what keeps them in sync):
//!
//! * **Linux** — PulseAudio protocol (PipeWire's `pipewire-pulse` on current Ubuntu, or
//!   PulseAudio): microphones are sources, *system audio* is the monitor of an output (sink).
//!   Devices come from `pactl -f json list sources`.
//! * **Windows** — DirectShow capture devices from `ffmpeg -list_devices`. System audio needs
//!   a loopback device ("Stereo Mix" or a virtual cable); WASAPI loopback without one is not
//!   available through FFmpeg and is listed as a known limitation.

use crate::error::{AppError, Result};
use crate::media::process::{command, run};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioKind {
    Microphone,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    /// What FFmpeg opens: a PulseAudio source name, or a DirectShow device name.
    pub id: String,
    pub name: String,
    pub kind: AudioKind,
    pub default: bool,
}

/// Audio in a recording. `None` = that source is off.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioOptions {
    pub microphone: Option<String>,
    pub system: Option<String>,
    /// Both sources on: separate tracks (edit them later) instead of one mixed track.
    pub separate_tracks: bool,
    /// Microphone volume in percent (0–200).
    pub mic_volume: u32,
    /// System audio volume in percent (0–200).
    pub system_volume: u32,
}

impl AudioOptions {
    pub fn is_empty(&self) -> bool {
        self.microphone.is_none() && self.system.is_none()
    }
}

// ------------------------------------------------------------------ devices

/// `pactl -f json list sources` → devices. Monitors of outputs are system audio.
pub fn parse_pactl_sources(json: &str, default_source: &str, default_sink: &str) -> Vec<AudioDevice> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    let mut out = Vec::new();
    for s in list {
        let Some(name) = s["name"].as_str() else { continue };
        let desc = s["description"].as_str().unwrap_or(name).to_string();
        // pactl names the field "monitor_of_sink" (PulseAudio) or "monitor_source" (15.99 with
        // PipeWire); either way it holds the output a monitor belongs to.
        let real = |m: &&str| !m.is_empty() && *m != "n/a";
        let monitor_of = s["monitor_of_sink"].as_str().filter(real).or_else(|| s["monitor_source"].as_str().filter(real).filter(|_| name.ends_with(".monitor")));
        if let Some(sink) = monitor_of {
            out.push(AudioDevice { id: name.into(), name: desc.trim_start_matches("Monitor of ").to_string(), kind: AudioKind::System, default: sink == default_sink });
        } else {
            out.push(AudioDevice { id: name.into(), name: desc, kind: AudioKind::Microphone, default: name == default_source });
        }
    }
    out
}

/// `ffmpeg -list_devices true -f dshow -i dummy` (stderr) → audio devices. Loopback-like names
/// ("Stereo Mix", "What U Hear", virtual cables) are offered as system audio.
pub fn parse_dshow(stderr: &str) -> Vec<AudioDevice> {
    let mut out: Vec<AudioDevice> = Vec::new();
    for line in stderr.lines() {
        // [dshow @ 000001] "Microphone (Realtek(R) Audio)" (audio)
        let Some(start) = line.find('"') else { continue };
        let rest = &line[start + 1..];
        let Some(end) = rest.find('"') else { continue };
        if !rest[end..].contains("(audio)") {
            continue;
        }
        let name = rest[..end].to_string();
        let lower = name.to_lowercase();
        let system = ["stereo mix", "what u hear", "loopback", "cable output", "wave out"].iter().any(|k| lower.contains(k));
        let kind = if system { AudioKind::System } else { AudioKind::Microphone };
        let default = !out.iter().any(|d| d.kind == kind);
        out.push(AudioDevice { id: name.clone(), name, kind, default });
    }
    out
}

fn text(program: &str, args: &[&str]) -> Option<String> {
    let out = command(Path::new(program)).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Microphones and system-audio sources available now.
pub fn list_devices(ffmpeg: Option<&Path>) -> Result<Vec<AudioDevice>> {
    if cfg!(windows) {
        let ffmpeg = ffmpeg.ok_or(AppError::FfmpegMissing)?;
        let args: Vec<String> = ["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"].iter().map(|s| s.to_string()).collect();
        let out = run(ffmpeg, &args, Duration::from_secs(10))?;
        return Ok(parse_dshow(&out.stderr));
    }
    let Some(json) = text("pactl", &["-f", "json", "list", "sources"]) else {
        // No pactl: the server's defaults still work through FFmpeg.
        return Ok(vec![
            AudioDevice { id: "default".into(), name: "Default microphone".into(), kind: AudioKind::Microphone, default: true },
            AudioDevice { id: "@DEFAULT_MONITOR@".into(), name: "System audio (default output)".into(), kind: AudioKind::System, default: true },
        ]);
    };
    let src = text("pactl", &["get-default-source"]).unwrap_or_default();
    let sink = text("pactl", &["get-default-sink"]).unwrap_or_default();
    Ok(parse_pactl_sources(&json, &src, &sink))
}

// ------------------------------------------------------------------ FFmpeg

fn s(v: &str) -> String {
    v.to_string()
}

/// FFmpeg input arguments for one device.
pub fn input_args(device: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![s("-thread_queue_size"), s("1024"), s("-f"), s("dshow"), s("-audio_buffer_size"), s("50"), s("-i"), format!("audio={device}")]
    } else {
        vec![s("-thread_queue_size"), s("1024"), s("-f"), s("pulse"), s("-sample_rate"), s("48000"), s("-channels"), s("2"), s("-fragment_size"), s("4096"), s("-i"), device.to_string()]
    }
}

fn volume(pct: u32) -> String {
    format!("volume={:.2}", pct.min(200) as f64 / 100.0)
}

/// The audio part of an FFmpeg command: inputs (after the video input), and the mapping,
/// filters and codec. `first_index` is the input number of the first audio input (1 after a
/// video input, 0 for audio-only). `meter` adds a level meter on stderr.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioPlan {
    pub inputs: Vec<String>,
    pub output: Vec<String>,
    pub tracks: u32,
}

pub fn plan(opts: &AudioOptions, first_index: usize, codec_args: &[&str], meter: bool) -> AudioPlan {
    let mut sources: Vec<(&str, u32)> = Vec::new();
    if let Some(m) = &opts.microphone {
        sources.push((m.as_str(), if opts.mic_volume == 0 && opts.system_volume == 0 { 100 } else { opts.mic_volume }));
    }
    if let Some(sy) = &opts.system {
        sources.push((sy.as_str(), if opts.mic_volume == 0 && opts.system_volume == 0 { 100 } else { opts.system_volume }));
    }
    if sources.is_empty() {
        return AudioPlan::default();
    }
    let mut p = AudioPlan::default();
    for (dev, _) in &sources {
        p.inputs.extend(input_args(dev));
    }
    // Level meter: a peak value every 0.1 s, printed through FFmpeg's log (at "info": a file
    // or pipe output would be buffered and arrive only at the end). The session parses it.
    let meter_f = if meter { ",asetnsamples=n=4800:p=0,astats=metadata=1:reset=1,ametadata=mode=print:key=lavfi.astats.Overall.Peak_level".to_string() } else { String::new() };
    let mut graph = Vec::new();
    if sources.len() == 2 && !opts.separate_tracks {
        let (a, b) = (first_index, first_index + 1);
        graph.push(format!("[{a}:a]{}[m0];[{b}:a]{}[m1];[m0][m1]amix=inputs=2:duration=longest:normalize=0,aresample=async=1{meter_f}[aout]", volume(sources[0].1), volume(sources[1].1)));
        p.output.extend([s("-filter_complex"), graph.join(";"), s("-map"), s("[aout]")]);
        p.tracks = 1;
    } else {
        for (i, (_, vol)) in sources.iter().enumerate() {
            let idx = first_index + i;
            let m = if i == 0 { meter_f.as_str() } else { "" };
            graph.push(format!("[{idx}:a]{},aresample=async=1{m}[a{i}]", volume(*vol)));
        }
        p.output.extend([s("-filter_complex"), graph.join(";")]);
        for i in 0..sources.len() {
            p.output.extend([s("-map"), format!("[a{i}]")]);
        }
        p.tracks = sources.len() as u32;
    }
    p.output.extend(codec_args.iter().map(|a| a.to_string()));
    p
}

/// Audio codec arguments for a video container.
pub fn codec_for(container: &str) -> &'static [&'static str] {
    match container {
        "webm" => &["-c:a", "libopus", "-b:a", "128k"],
        _ => &["-c:a", "aac", "-b:a", "160k"],
    }
}

// ------------------------------------------------------------------ audio-only recordings

pub const AUDIO_FORMATS: &[&str] = &["mp3", "m4a", "opus", "flac", "wav"];

/// Codec arguments for an audio-only file, and its container name for the segment-join.
pub fn audio_format(format: &str, quality: &str) -> Result<(Vec<String>, &'static str)> {
    let kbps = |hi: u32, mid: u32, lo: u32| format!("{}k", match quality { "high" => hi, "small" => lo, _ => mid });
    let args = match format {
        "mp3" => vec![s("-c:a"), s("libmp3lame"), s("-b:a"), kbps(256, 192, 128)],
        "m4a" => vec![s("-c:a"), s("aac"), s("-b:a"), kbps(256, 160, 96)],
        "opus" => vec![s("-c:a"), s("libopus"), s("-b:a"), kbps(160, 112, 64)],
        "flac" => vec![s("-c:a"), s("flac")],
        "wav" => vec![s("-c:a"), s("pcm_s16le")],
        other => return Err(AppError::InvalidSetting(format!("unknown audio format {other}"))),
    };
    let ext = match format {
        "mp3" => "mp3",
        "m4a" => "m4a",
        "opus" => "opus",
        "flac" => "flac",
        _ => "wav",
    };
    Ok((args, ext))
}

/// A peak level printed by the meter ("lavfi.astats.Overall.Peak_level=-12.3"), in dBFS.
pub fn parse_level(line: &str) -> Option<f64> {
    let v = line.split_once("lavfi.astats.Overall.Peak_level=")?.1.trim();
    if v.starts_with("-inf") {
        return Some(-120.0);
    }
    v.parse::<f64>().ok().map(|db| db.clamp(-120.0, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pactl_sources() {
        let json = r#"[{"name":"alsa_output.pci.analog-stereo.monitor","description":"Monitor of Built-in Audio Analog Stereo","monitor_of_sink":"alsa_output.pci.analog-stereo"},
            {"name":"alsa_input.pci.analog-stereo","description":"Built-in Audio Analog Stereo","monitor_of_sink":null},
            {"name":"bluez_input.headset","description":"Headset","monitor_of_sink":"n/a"},
            {"name":"speaker.monitor","description":"Monitor of speaker","monitor_source":"speaker"}]"#;
        let d = parse_pactl_sources(json, "alsa_input.pci.analog-stereo", "alsa_output.pci.analog-stereo");
        assert!(!d[3].default);
        assert_eq!(d.len(), 4);
        assert_eq!(d[3].kind, AudioKind::System);
        assert_eq!((d[0].kind, d[0].default, d[0].name.as_str()), (AudioKind::System, true, "Built-in Audio Analog Stereo"));
        assert_eq!((d[1].kind, d[1].default), (AudioKind::Microphone, true));
        assert_eq!((d[2].kind, d[2].default), (AudioKind::Microphone, false));
        assert!(parse_pactl_sources("not json", "", "").is_empty());
    }

    #[test]
    fn dshow_devices() {
        let err = "[dshow @ 0x1] \"Integrated Camera\" (video)\n[dshow @ 0x1]   Alternative name \"@device_pnp\"\n[dshow @ 0x1] \"Microphone (Realtek(R) Audio)\" (audio)\n[dshow @ 0x1] \"Stereo Mix (Realtek(R) Audio)\" (audio)\n";
        let d = parse_dshow(err);
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].kind, d[0].default), (AudioKind::Microphone, true));
        assert_eq!(d[1].kind, AudioKind::System);
    }

    #[test]
    fn plans() {
        let both = AudioOptions { microphone: Some("mic".into()), system: Some("mon".into()), separate_tracks: false, mic_volume: 100, system_volume: 80 };
        let p = plan(&both, 1, codec_for("mp4"), false);
        assert_eq!(p.tracks, 1);
        let fc = &p.output[p.output.iter().position(|a| a == "-filter_complex").unwrap() + 1];
        assert!(fc.contains("[1:a]volume=1.00") && fc.contains("[2:a]volume=0.80") && fc.contains("amix=inputs=2"), "{fc}");
        assert!(p.output.ends_with(&["-c:a".to_string(), "aac".into(), "-b:a".into(), "160k".into()]));
        let sep = plan(&AudioOptions { separate_tracks: true, ..both.clone() }, 1, codec_for("webm"), false);
        assert_eq!(sep.tracks, 2);
        assert_eq!(sep.output.iter().filter(|a| *a == "-map").count(), 2);
        assert!(plan(&AudioOptions::default(), 1, codec_for("mp4"), false).inputs.is_empty());
        let only = plan(&AudioOptions { microphone: Some("mic".into()), ..Default::default() }, 0, &["-c:a", "flac"], true);
        assert!(only.output[1].starts_with("[0:a]volume=1.00") && only.output[1].contains("astats"), "{:?}", only.output);
    }

    #[test]
    fn levels() {
        assert_eq!(parse_level("lavfi.astats.Overall.Peak_level=-12.5"), Some(-12.5));
        assert_eq!(parse_level("lavfi.astats.Overall.Peak_level=-inf"), Some(-120.0));
        assert_eq!(parse_level("frame:3 pts:1"), None);
    }

    #[test]
    fn formats() {
        assert_eq!(audio_format("mp3", "high").unwrap().0[3], "256k");
        assert_eq!(audio_format("flac", "balanced").unwrap().1, "flac");
        assert!(audio_format("ogg", "high").is_err());
    }
}
