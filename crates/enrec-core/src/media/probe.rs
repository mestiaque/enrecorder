//! FFprobe metadata: container, streams, duration, resolution, frame rate, bitrate.
//! Also used to verify every finished recording (a file without a readable video stream or
//! with no duration is a failed recording, never a "completed" one).

use super::process::{run, tail};
use crate::error::{AppError, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub fps: Option<f64>,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub video_streams: i64,
    pub audio_streams: i64,
    /// The full ffprobe JSON, for the metadata inspector.
    pub raw: String,
}

/// "30000/1001" → 29.97; "0/0" → None.
pub fn parse_rate(s: &str) -> Option<f64> {
    let (n, d) = s.split_once('/').unwrap_or((s, "1"));
    let (n, d): (f64, f64) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
    (d > 0.0 && n > 0.0).then(|| (n / d * 1000.0).round() / 1000.0)
}

/// Parses `ffprobe -show_format -show_streams -of json` output.
pub fn parse(json: &str) -> Result<Probe> {
    let v: Value = serde_json::from_str(json)?;
    let mut p = Probe { raw: json.to_string(), ..Probe::default() };
    let num = |v: &Value| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_f64());
    if let Some(f) = v.get("format") {
        p.container = f["format_name"].as_str().map(str::to_string);
        p.duration_ms = num(&f["duration"]).map(|s| (s * 1000.0).round() as i64);
        p.bitrate = num(&f["bit_rate"]).map(|b| b as i64);
    }
    for s in v["streams"].as_array().into_iter().flatten() {
        match s["codec_type"].as_str() {
            Some("video") if s["disposition"]["attached_pic"].as_i64() != Some(1) => {
                p.video_streams += 1;
                if p.video_codec.is_none() {
                    p.video_codec = s["codec_name"].as_str().map(str::to_string);
                    p.width = s["width"].as_i64();
                    p.height = s["height"].as_i64();
                    p.fps = s["avg_frame_rate"].as_str().and_then(parse_rate).or_else(|| s["r_frame_rate"].as_str().and_then(parse_rate));
                }
                if p.duration_ms.is_none() {
                    p.duration_ms = num(&s["duration"]).map(|d| (d * 1000.0).round() as i64);
                }
            }
            Some("audio") => {
                p.audio_streams += 1;
                if p.audio_codec.is_none() {
                    p.audio_codec = s["codec_name"].as_str().map(str::to_string);
                }
            }
            _ => {}
        }
    }
    Ok(p)
}

pub fn probe(ffprobe: &Path, file: &Path) -> Result<Probe> {
    let args = ["-v", "error", "-show_format", "-show_streams", "-of", "json"].iter().map(|s| s.to_string()).chain([file.to_string_lossy().into_owned()]).collect::<Vec<_>>();
    let out = run(ffprobe, &args, Duration::from_secs(30))?;
    if !out.success {
        return Err(AppError::Ffmpeg(format!("ffprobe could not read {}: {}", file.display(), tail(&out.stderr, 2))));
    }
    parse(&out.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates() {
        assert_eq!(parse_rate("30/1"), Some(30.0));
        assert_eq!(parse_rate("30000/1001"), Some(29.97));
        assert_eq!(parse_rate("0/0"), None);
        assert_eq!(parse_rate("x"), None);
    }

    #[test]
    fn parses_streams_and_format() {
        let j = r#"{"streams":[
            {"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30/1"},
            {"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1}},
            {"codec_type":"audio","codec_name":"aac"},
            {"codec_type":"audio","codec_name":"opus"}],
          "format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2","duration":"12.345","bit_rate":"2500000"}}"#;
        let p = parse(j).unwrap();
        assert_eq!(p.video_codec.as_deref(), Some("h264"));
        assert_eq!((p.width, p.height, p.fps), (Some(1920), Some(1080), Some(30.0)));
        assert_eq!((p.video_streams, p.audio_streams), (1, 2));
        assert_eq!(p.audio_codec.as_deref(), Some("aac"));
        assert_eq!(p.duration_ms, Some(12345));
        assert_eq!(p.bitrate, Some(2_500_000));
    }

    #[test]
    fn duration_from_stream_when_format_has_none() {
        let p = parse(r#"{"streams":[{"codec_type":"video","codec_name":"vp9","duration":"2.5"}],"format":{"format_name":"matroska,webm"}}"#).unwrap();
        assert_eq!(p.duration_ms, Some(2500));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("not json").is_err());
    }
}
