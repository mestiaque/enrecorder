//! Optional transcription (whisper.cpp): status, start with progress events, cancel.

use super::blocking;
use crate::state::AppState;
use enrec_core::media::transcribe;
use enrec_core::{AppError, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub const EV_TRANSCRIBE: &str = "transcribe-progress";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionStatus {
    /// whisper.cpp program, when found.
    program: Option<String>,
    /// Speech models found (the chosen one first).
    models: Vec<String>,
    /// Where models are looked for (shown in the help text).
    model_folder: String,
    /// Media id being transcribed now.
    running: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    media_id: String,
    percent: u32,
}

fn custom(s: &str) -> Option<PathBuf> {
    (!s.is_empty()).then(|| PathBuf::from(s))
}

#[tauri::command]
pub async fn transcription_status(app: AppHandle) -> Result<TranscriptionStatus> {
    blocking(move || {
        let state = app.state::<AppState>();
        let s = state.settings.read().clone();
        let program = transcribe::find_whisper(custom(&s.whisper_path).as_deref(), &state.data_dir);
        let models = transcribe::find_models(custom(&s.whisper_model).as_deref(), &transcribe::model_dirs(&state.data_dir));
        let running = state.transcribing.lock().as_ref().map(|(id, _)| id.clone());
        Ok(TranscriptionStatus {
            program: program.map(|p| p.to_string_lossy().into_owned()),
            models: models.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
            model_folder: state.data_dir.join("whisper").to_string_lossy().into_owned(),
            running,
        })
    })
    .await
}

/// Writes "<name>.srt" next to the media file; returns its path.
#[tauri::command]
pub async fn transcribe_media(app: AppHandle, id: String) -> Result<String> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let state = app.state::<AppState>();
        let mut t = state.transcribing.lock();
        if t.is_some() {
            return Err(AppError::InvalidSetting("a transcription is already running — wait for it or cancel it".into()));
        }
        *t = Some((id.clone(), cancel.clone()));
    }
    let app2 = app.clone();
    let r = blocking(move || {
        let state = app2.state::<AppState>();
        let s = state.settings.read().clone();
        let item = state.db.media_item(&id)?;
        if item.kind == "screenshot" {
            return Err(AppError::InvalidSetting("screenshots have no sound".into()));
        }
        let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
        let program = transcribe::find_whisper(custom(&s.whisper_path).as_deref(), &state.data_dir)
            .ok_or_else(|| AppError::Unsupported("whisper.cpp is not installed — see Settings → Tools → Transcription".into()))?;
        let model = transcribe::find_models(custom(&s.whisper_model).as_deref(), &transcribe::model_dirs(&state.data_dir))
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Unsupported("no speech model (ggml-*.bin) found — see Settings → Tools → Transcription".into()))?;
        log::info!("transcribing {} with {} ({})", item.path, program.display(), model.display());
        let work = state.data_dir.join("tmp");
        let emit = |p: u32| {
            let _ = app2.emit(EV_TRANSCRIBE, Progress { media_id: id.clone(), percent: p });
        };
        let out = transcribe::transcribe(Path::new(&ffmpeg.path), &program, &model, Path::new(&item.path), &s.transcribe_language, &work, &cancel, &emit)?;
        log::info!("transcript saved: {}", out.display());
        Ok(out.to_string_lossy().into_owned())
    })
    .await;
    *app.state::<AppState>().transcribing.lock() = None;
    if let Ok(p) = &r {
        let name = Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        super::recording::notify(&app, "Transcript ready", &name);
    }
    r
}

#[tauri::command]
pub fn cancel_transcription(state: tauri::State<AppState>) {
    if let Some((_, c)) = state.transcribing.lock().as_ref() {
        c.store(true, Ordering::Release);
    }
}
