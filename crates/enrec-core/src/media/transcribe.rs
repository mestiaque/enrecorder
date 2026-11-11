//! Optional transcription with whisper.cpp (a separate program plus a speech model file, both
//! chosen by the user or found in the usual places). FFmpeg turns the media's sound into 16 kHz
//! mono WAV, `whisper-cli` writes an `.srt` next to the media. Everything stays on this computer;
//! when whisper.cpp or a model is missing the app says so instead of pretending.

use crate::error::{AppError, Result};
use crate::media::process::{command, run, tail};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Program names whisper.cpp has used ("main" is too generic to look for on PATH; plain
/// "whisper" is usually the Python package with different options).
const PROGRAMS: [&str; 2] = ["whisper-cli", "whisper-cpp"];

/// The whisper.cpp program: the chosen path, then `<data>/whisper/`, then PATH.
pub fn find_whisper(custom: Option<&Path>, data_dir: &Path) -> Option<PathBuf> {
    if let Some(c) = custom.filter(|c| !c.as_os_str().is_empty()) {
        return c.is_file().then(|| c.to_path_buf());
    }
    let mut dirs = vec![data_dir.join("whisper")];
    dirs.extend(std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default());
    for d in dirs {
        for p in PROGRAMS {
            let f = d.join(if cfg!(windows) { format!("{p}.exe") } else { p.to_string() });
            if f.is_file() {
                return Some(f);
            }
        }
    }
    None
}

/// Folders searched for `ggml-*.bin` speech models.
pub fn model_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let mut v = vec![data_dir.join("whisper")];
    if let Some(d) = dirs_next_data() {
        v.push(d.join("whisper.cpp").join("models"));
    }
    if !cfg!(windows) {
        v.push(PathBuf::from("/usr/share/whisper.cpp/models"));
        v.push(PathBuf::from("/usr/local/share/whisper.cpp/models"));
    }
    v
}

fn dirs_next_data() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}

/// Speech models found in `dirs` (plus the chosen one), smallest first.
pub fn find_models(custom: Option<&Path>, dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if let Some(c) = custom.filter(|c| c.is_file()) {
        v.push(c.to_path_buf());
    }
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        let mut found: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("ggml-") && n.ends_with(".bin")))
            .collect();
        found.sort_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0));
        for f in found {
            if !v.contains(&f) {
                v.push(f);
            }
        }
    }
    v
}

/// An English-only model ("ggml-base.en.bin") cannot transcribe other languages.
pub fn english_only(model: &Path) -> bool {
    model.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(".en.") || n.contains(".en-"))
}

/// `progress = 35%` lines printed with `-pp`.
pub fn parse_progress(line: &str) -> Option<u32> {
    let rest = line.split("progress =").nth(1)?;
    rest.trim().trim_end_matches('%').trim().parse::<u32>().ok().map(|p| p.min(100))
}

/// `name.srt` next to `media`; "name (2).srt" when that exists.
pub fn srt_path(media: &Path) -> PathBuf {
    let dir = media.parent().unwrap_or(Path::new("."));
    let stem = media.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "transcript".into());
    let mut p = dir.join(format!("{stem}.srt"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{stem} ({n}).srt"));
        n += 1;
    }
    p
}

/// Arguments for `whisper-cli`: model, WAV in, SRT out (`out_base` + ".srt"), language.
pub fn whisper_args(model: &Path, wav: &Path, out_base: &Path, language: &str, threads: usize) -> Vec<String> {
    let lang = if language.trim().is_empty() { "auto" } else { language.trim() };
    vec![
        "-m".into(),
        model.to_string_lossy().into_owned(),
        "-f".into(),
        wav.to_string_lossy().into_owned(),
        "-l".into(),
        lang.into(),
        "-t".into(),
        threads.clamp(1, 16).to_string(),
        "-osrt".into(),
        "-of".into(),
        out_base.to_string_lossy().into_owned(),
        "-pp".into(),
        "-np".into(),
    ]
}

/// Number of subtitle blocks in an SRT.
pub fn count_cues(srt: &str) -> usize {
    srt.lines().filter(|l| l.contains(" --> ")).count()
}

/// Transcribes `media` into an SRT next to it and returns its path. `progress` gets 0–100;
/// setting `cancel` stops whisper.cpp and removes the partial file.
#[allow(clippy::too_many_arguments)]
pub fn transcribe(ffmpeg: &Path, whisper: &Path, model: &Path, media: &Path, language: &str, work_dir: &Path, cancel: &AtomicBool, progress: &dyn Fn(u32)) -> Result<PathBuf> {
    if !media.is_file() {
        return Err(AppError::NotFound(format!("{} is missing", media.display())));
    }
    if english_only(model) && !matches!(language.trim(), "" | "auto" | "en") {
        return Err(AppError::InvalidSetting(format!("{} only understands English — choose a multilingual model", model.file_name().unwrap_or_default().to_string_lossy())));
    }
    std::fs::create_dir_all(work_dir)?;
    let id = crate::database::new_id();
    let wav = work_dir.join(format!("transcribe-{id}.wav"));
    let base = work_dir.join(format!("transcribe-{id}"));
    let cleanup = || {
        let _ = std::fs::remove_file(&wav);
        let _ = std::fs::remove_file(base.with_extension("srt"));
    };
    progress(0);
    let args: Vec<String> = ["-hide_banner", "-nostdin", "-loglevel", "error", "-i", &media.to_string_lossy(), "-map", "0:a:0", "-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", "-y", &wav.to_string_lossy()]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let r = run(ffmpeg, &args, Duration::from_secs(3600))?;
    if !r.success {
        cleanup();
        let why = tail(&r.stderr, 2);
        return Err(if why.contains("matches no streams") { AppError::InvalidSetting("this file has no sound to transcribe".into()) } else { AppError::Ffmpeg(format!("could not read the sound: {why}")) });
    }
    if cancel.load(Ordering::Acquire) {
        cleanup();
        return Err(AppError::Cancelled);
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).max(1);
    let mut child = command(whisper)
        .args(whisper_args(model, &wav, &base, language, threads))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Io(format!("could not start {}: {e}", whisper.display())))?;
    // whisper.cpp prints progress (and its log) on stderr; keep the tail for errors.
    let stderr = child.stderr.take();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        if let Some(e) = stderr {
            for line in BufReader::new(e).lines().map_while(std::result::Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        }
    });
    let mut log: Vec<String> = Vec::new();
    let status = loop {
        while let Ok(line) = rx.try_recv() {
            if let Some(p) = parse_progress(&line) {
                progress(p.min(99));
            } else if !line.trim().is_empty() {
                log.push(line);
                if log.len() > 40 {
                    log.remove(0);
                }
            }
        }
        if cancel.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            // Not joined: a helper process could still hold the pipe; the thread ends with it.
            drop(reader);
            cleanup();
            return Err(AppError::Cancelled);
        }
        if let Some(st) = child.try_wait()? {
            break st;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = reader.join();
    for line in rx.try_iter() {
        if parse_progress(&line).is_none() {
            log.push(line);
        }
    }
    let _ = std::fs::remove_file(&wav);
    let made = base.with_extension("srt");
    if !status.success() || !made.is_file() {
        cleanup();
        let why = tail(&log.join("\n"), 2);
        return Err(AppError::Other(format!("whisper.cpp could not transcribe the file{}", if why.is_empty() { String::new() } else { format!(": {why}") })));
    }
    let out = srt_path(media);
    if std::fs::rename(&made, &out).is_err() {
        // Different drive: copy, then remove.
        std::fs::copy(&made, &out)?;
        let _ = std::fs::remove_file(&made);
    }
    progress(100);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines() {
        assert_eq!(parse_progress("whisper_print_progress_callback: progress =  35%"), Some(35));
        assert_eq!(parse_progress("progress = 100%"), Some(100));
        assert_eq!(parse_progress("whisper_init_from_file: loading model"), None);
    }

    #[test]
    fn srt_names_do_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("Talk.mp4");
        assert_eq!(srt_path(&media), dir.path().join("Talk.srt"));
        std::fs::write(dir.path().join("Talk.srt"), "x").unwrap();
        assert_eq!(srt_path(&media), dir.path().join("Talk (2).srt"));
    }

    #[test]
    fn args_and_models() {
        let a = whisper_args(Path::new("/m/ggml-base.bin"), Path::new("/t/a.wav"), Path::new("/t/a"), "", 64);
        assert!(a.windows(2).any(|w| w == ["-l", "auto"]));
        assert!(a.windows(2).any(|w| w == ["-t", "16"]));
        assert!(a.windows(2).any(|w| w == ["-of", "/t/a"]));
        assert!(a.contains(&"-osrt".to_string()));
        assert!(english_only(Path::new("ggml-tiny.en.bin")));
        assert!(english_only(Path::new("ggml-base.en-q5_1.bin")));
        assert!(!english_only(Path::new("ggml-large-v3.bin")));
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ggml-big.bin"), vec![0u8; 100]).unwrap();
        std::fs::write(dir.path().join("ggml-small.bin"), vec![0u8; 10]).unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        let m = find_models(None, &[dir.path().to_path_buf()]);
        assert_eq!(m.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect::<Vec<_>>(), ["ggml-small.bin", "ggml-big.bin"]);
        assert_eq!(count_cues("1\n00:00:00,000 --> 00:00:02,000\nHi\n\n2\n00:00:02,000 --> 00:00:03,000\nyo\n"), 2);
    }

    #[test]
    fn missing_whisper_and_wrong_language() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_whisper(Some(&dir.path().join("nope")), dir.path()).is_none());
        let media = dir.path().join("a.wav");
        std::fs::write(&media, "x").unwrap();
        let never = AtomicBool::new(false);
        let r = transcribe(Path::new("ffmpeg"), Path::new("whisper-cli"), Path::new("ggml-tiny.en.bin"), &media, "bn", dir.path(), &never, &|_| {});
        assert!(matches!(r, Err(AppError::InvalidSetting(_))));
    }

    /// The whole pipeline with real FFmpeg and a stand-in `whisper-cli` (a shell script that
    /// checks the 16 kHz mono WAV, prints progress like whisper.cpp v1.7 and writes `<-of>.srt`):
    /// progress, naming, temporary files, failure and cancel.
    #[cfg(unix)]
    #[test]
    fn pipeline_with_a_stand_in_whisper() {
        use std::os::unix::fs::PermissionsExt;
        let Some(ff) = crate::media::ffmpeg::find_tool("ffmpeg", None, &[]) else { return };
        let ff = PathBuf::from(&ff.path);
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("whisper-cli");
        std::fs::write(
            &fake,
            r#"#!/bin/sh
while [ $# -gt 0 ]; do case "$1" in -f) wav="$2"; shift;; -of) out="$2"; shift;; -m) model="$2"; shift;; esac; shift; done
[ -f "$model" ] || { echo "error: failed to initialize whisper context" >&2; exit 3; }
head -c 44 "$wav" | od -An -tu4 -j24 -N4 | grep -q 16000 || { echo "error: not 16 kHz" >&2; exit 4; }
[ "$(head -c 44 "$wav" | od -An -tu2 -j22 -N2 | tr -d ' ')" = 1 ] || { echo "error: not mono" >&2; exit 5; }
case "$model" in *slow*) exec sleep 30;; esac
for p in 5 50 95; do echo "whisper_print_progress_callback: progress = $p%" >&2; done
printf '1\n00:00:00,000 --> 00:00:01,500\n Hello there.\n\n' > "$out.srt"
"#,
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let model = dir.path().join("ggml-base.bin");
        std::fs::write(&model, "m").unwrap();
        let media = dir.path().join("Talk.mkv");
        let ok = crate::media::process::command(&ff)
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=1:sample_rate=48000", "-f", "lavfi", "-i", "color=black:size=64x64:duration=1", "-ac", "2", "-y"])
            .arg(&media)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let work = dir.path().join("work");
        let seen = std::sync::Mutex::new(Vec::new());
        let never = AtomicBool::new(false);
        let out = transcribe(&ff, &fake, &model, &media, "auto", &work, &never, &|p| seen.lock().unwrap().push(p)).unwrap();
        assert_eq!(out, dir.path().join("Talk.srt"));
        assert_eq!(count_cues(&std::fs::read_to_string(&out).unwrap()), 1);
        assert_eq!(*seen.lock().unwrap(), [0, 5, 50, 95, 100]);
        assert!(std::fs::read_dir(&work).unwrap().next().is_none(), "temporary files left behind");
        // A second run does not overwrite the first transcript.
        assert_eq!(transcribe(&ff, &fake, &model, &media, "en", &work, &never, &|_| {}).unwrap(), dir.path().join("Talk (2).srt"));
        // whisper.cpp fails (bad model): its message reaches the user, nothing is left behind.
        let err = transcribe(&ff, &fake, &dir.path().join("missing.bin"), &media, "auto", &work, &never, &|_| {}).unwrap_err();
        assert!(err.to_string().contains("failed to initialize whisper context"), "{err}");
        // A file without sound.
        let silent = dir.path().join("silent.mkv");
        assert!(crate::media::process::command(&ff).args(["-loglevel", "error", "-f", "lavfi", "-i", "color=black:size=64x64:duration=1", "-y"]).arg(&silent).status().unwrap().success());
        assert!(matches!(transcribe(&ff, &fake, &model, &silent, "auto", &work, &never, &|_| {}), Err(AppError::InvalidSetting(_))));
        // Cancel while whisper.cpp runs: it is stopped quickly and no transcript appears.
        let slow = dir.path().join("ggml-slow.bin");
        std::fs::write(&slow, "m").unwrap();
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let c2 = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            c2.store(true, Ordering::Release);
        });
        let t = std::time::Instant::now();
        assert!(matches!(transcribe(&ff, &fake, &slow, &media, "auto", &work, &cancel, &|_| {}), Err(AppError::Cancelled)));
        assert!(t.elapsed() < Duration::from_secs(5));
        assert!(!dir.path().join("Talk (3).srt").exists());
        assert!(std::fs::read_dir(&work).unwrap().next().is_none());
    }

    /// Real whisper.cpp (skipped unless ENREC_WHISPER, ENREC_WHISPER_MODEL and ENREC_SPEECH
    /// — a WAV of someone saying something with "hello" in it — are set).
    #[test]
    fn transcribes_real_speech_if_available() {
        let (Some(w), Some(m), Some(speech)) = (std::env::var_os("ENREC_WHISPER"), std::env::var_os("ENREC_WHISPER_MODEL"), std::env::var_os("ENREC_SPEECH")) else { return };
        let Some(ff) = crate::media::ffmpeg::find_tool("ffmpeg", None, &[]) else { return };
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("speech.wav");
        std::fs::copy(&speech, &media).unwrap();
        let seen = std::sync::Mutex::new(Vec::new());
        let never = AtomicBool::new(false);
        let out = transcribe(Path::new(&ff.path), Path::new(&w), Path::new(&m), &media, "auto", &dir.path().join("work"), &never, &|p| seen.lock().unwrap().push(p)).unwrap();
        let srt = std::fs::read_to_string(&out).unwrap();
        assert_eq!(out, dir.path().join("speech.srt"));
        assert!(count_cues(&srt) >= 1, "{srt}");
        assert!(srt.to_lowercase().contains("hello"), "{srt}");
        assert_eq!(seen.lock().unwrap().last(), Some(&100));
        assert!(std::fs::read_dir(dir.path().join("work")).unwrap().next().is_none(), "temporary files left behind");
        // Cancelled before whisper starts: nothing is written.
        let cancel = AtomicBool::new(true);
        assert!(matches!(transcribe(Path::new(&ff.path), Path::new(&w), Path::new(&m), &media, "auto", &dir.path().join("work"), &cancel, &|_| {}), Err(AppError::Cancelled)));
        assert!(!dir.path().join("speech (2).srt").exists());
    }
}
