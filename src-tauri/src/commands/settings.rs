use super::tauri_err;
use crate::state::AppState;
use enrec_core::settings::{check_output_folder, Settings, FOLDER_KEYS};
use enrec_core::{AppError, Result};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    settings: Settings,
    /// Folders used while `recordingsDir` / `screenshotsDir` are empty.
    default_recordings: String,
    default_screenshots: String,
}

fn payload(state: &AppState) -> SettingsPayload {
    let d = Settings::default();
    SettingsPayload {
        settings: state.settings.read().clone(),
        default_recordings: d.recordings_folder(&state.system_videos).to_string_lossy().into_owned(),
        default_screenshots: d.screenshots_folder(&state.system_pictures).to_string_lossy().into_owned(),
    }
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> SettingsPayload {
    payload(&state)
}

/// Validates and stores one setting; returns the stored settings.
#[tauri::command]
pub fn set_setting(app: AppHandle, state: State<AppState>, key: String, value: Value) -> Result<Settings> {
    if !Settings::is_known_key(&key) {
        return Err(AppError::InvalidSetting(format!("unknown setting {key}")));
    }
    if FOLDER_KEYS.contains(&key.as_str()) {
        check_output_folder(value.as_str().unwrap_or_default())?;
    }
    if key == "ffmpegPath" {
        let p = value.as_str().unwrap_or_default().trim();
        if !p.is_empty() && !std::path::Path::new(p).is_file() {
            return Err(AppError::InvalidPath(format!("{p} is not a file")));
        }
    }
    let mut map = state.db.settings_map()?;
    map.insert(key.clone(), value.clone());
    let next = Settings::from_map(&map);
    let stored = serde_json::to_value(&next)?.get(&key).cloned().unwrap_or(value);
    state.db.set_setting(&key, &stored)?;
    *state.settings.write() = next.clone();
    match key.as_str() {
        "startWithOs" => apply_autostart(&app, next.start_with_os)?,
        "ffmpegPath" => super::media::refresh_tools(app.clone(), false),
        _ => {}
    }
    if FOLDER_KEYS.contains(&key.as_str()) {
        crate::allow_media_dirs(&app);
    }
    Ok(next)
}

pub fn apply_autostart(app: &AppHandle, on: bool) -> Result<()> {
    let al = app.autolaunch();
    let enabled = al.is_enabled().unwrap_or(false);
    if on && !enabled {
        al.enable().map_err(tauri_err)?;
    } else if !on && enabled {
        al.disable().map_err(tauri_err)?;
    }
    Ok(())
}

#[tauri::command]
pub fn reset_settings(app: AppHandle, state: State<AppState>) -> Result<SettingsPayload> {
    let had_custom_ffmpeg = !state.settings.read().ffmpeg_path.is_empty();
    state.db.replace_settings(&serde_json::Map::new())?;
    *state.settings.write() = Settings::default();
    apply_autostart(&app, false)?;
    if had_custom_ffmpeg {
        super::media::refresh_tools(app.clone(), false);
    }
    Ok(payload(&app.state::<AppState>()))
}

// ------------------------------------------------------------------ shortcuts

#[tauri::command]
pub fn get_shortcuts(state: State<AppState>) -> Result<Vec<enrec_core::database::shortcuts::ShortcutRow>> {
    state.db.shortcuts()
}

/// Changes one action's shortcut (`None` turns it off); refused when another action or the
/// system already uses it.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, action: String, accelerator: Option<String>) -> Result<Vec<enrec_core::database::shortcuts::ShortcutRow>> {
    let state = app.state::<AppState>();
    let before = state.db.shortcuts()?;
    state.db.set_shortcut(&action, accelerator.as_deref())?;
    if let Err(e) = crate::register_shortcuts(&app) {
        // Another program holds it: go back to what worked.
        for row in before {
            let _ = state.db.set_shortcut(&row.action, row.accelerator.as_deref());
        }
        let _ = crate::register_shortcuts(&app);
        return Err(e);
    }
    state.db.shortcuts()
}

#[tauri::command]
pub fn reset_shortcuts(app: AppHandle) -> Result<Vec<enrec_core::database::shortcuts::ShortcutRow>> {
    let state = app.state::<AppState>();
    state.db.reset_shortcuts()?;
    crate::register_shortcuts(&app)?;
    state.db.shortcuts()
}
