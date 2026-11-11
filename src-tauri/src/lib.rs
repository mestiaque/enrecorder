mod commands;
mod state;
mod tray;

use commands::{dashboard, library, media, recording, settings, system};
use enrec_core::database::Db;
use enrec_core::settings::Settings;
use parking_lot::RwLock;
use state::{AppState, MediaTools};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, WindowEvent};

/// Quit for real: a running recording is stopped and saved first (off the main thread, so the
/// window keeps drawing), then the app exits.
pub fn quit(app: &tauri::AppHandle) {
    app.state::<AppState>().quitting.store(true, Ordering::SeqCst);
    log::info!("quitting");
    let app = app.clone();
    std::thread::spawn(move || {
        commands::recording::stop_and_wait(&app, std::time::Duration::from_secs(90));
        app.exit(0);
    });
}

/// The capture backend for the display server of this session.
fn capture_backend(sys: &enrec_core::system::SystemInfo) -> Result<std::sync::Arc<dyn enrec_core::capture::CaptureBackend>, String> {
    use enrec_core::system::DisplayServer;
    match sys.display_server {
        #[cfg(target_os = "linux")]
        DisplayServer::X11 => enrec_core::capture::x11::X11Backend::new().map(|b| std::sync::Arc::new(b) as _).map_err(|e| e.to_string()),
        #[cfg(target_os = "linux")]
        DisplayServer::Wayland => enrec_core::capture::wayland::WaylandBackend::new().map(|b| std::sync::Arc::new(b) as _).map_err(|e| e.to_string()),
        #[cfg(target_os = "windows")]
        DisplayServer::Windows => enrec_core::capture::windows::WindowsBackend::new().map(|b| std::sync::Arc::new(b) as _).map_err(|e| e.to_string()),
        _ => Err("No graphical session was found, so the screen cannot be recorded.".into()),
    }
}

fn build_state(app: &tauri::AppHandle) -> Result<AppState, Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let log_dir = app.path().app_log_dir()?;
    let home = app.path().home_dir()?;
    let system_videos = app.path().video_dir().unwrap_or_else(|_| home.join("Videos"));
    let system_pictures = app.path().picture_dir().unwrap_or_else(|_| home.join("Pictures"));
    let db = Db::open(&data_dir.join("enrecorder.db"))?;
    let interrupted = db.mark_interrupted_sessions()?;
    if interrupted > 0 {
        log::warn!("{interrupted} recording session(s) were interrupted last time; their files are kept");
    }
    let settings = Settings::from_map(&db.settings_map()?);
    let system = enrec_core::system::detect();
    let capture = capture_backend(&system);
    if let Err(why) = &capture {
        log::warn!("screen capture unavailable: {why}");
    }
    Ok(AppState {
        db: std::sync::Arc::new(db),
        settings: RwLock::new(settings),
        system: RwLock::new(system),
        capture,
        recorder: parking_lot::Mutex::new(None),
        tools: RwLock::new(MediaTools::default()),
        quitting: AtomicBool::new(false),
        data_dir,
        log_dir,
        system_videos,
        system_pictures,
    })
}

/// Ctrl+Shift+R start/stop, Ctrl+Shift+P pause/resume (work while the window is hidden).
fn register_shortcuts(app: &tauri::AppHandle) {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
    for (code, what) in [(Code::KeyR, "start/stop"), (Code::KeyP, "pause/resume")] {
        if let Err(e) = app.global_shortcut().register(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), code)) {
            log::warn!("shortcut for {what} is not available (another app may use it): {e}");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        // First plugin: a second launch only brings the running window to the front.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| tray::show_main(app)))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info })
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: Some("enrecorder".into()) }),
                ])
                .max_file_size(5 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .format(|out, message, record| {
                    let now = tauri_plugin_log::TimezoneStrategy::UseLocal
                        .get_now()
                        .format(time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"))
                        .unwrap_or_default();
                    out.finish(format_args!("[{now}][{}][{}] {}", record.level(), record.target(), message))
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    log::info!("global shortcut {shortcut:?}");
                    if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyR) {
                        recording::toggle_record(app);
                    } else if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyP) {
                        recording::toggle_pause(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let state = build_state(app.handle())?;
            let hidden = std::env::args().any(|a| a == "--minimized") || state.settings.read().start_minimized;
            let s = state.system.read().clone();
            log::info!("ENRecorder {} on {} · display {:?} · desktop {}", app.package_info().version, s.os_name, s.display_server, s.desktop);
            app.manage(state);
            media::refresh_tools(app.handle().clone(), false);
            tray::install(app.handle())?;
            register_shortcuts(app.handle());
            tray::fit_to_monitor(app.handle());
            if !hidden {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                if state.quitting.load(Ordering::SeqCst) {
                    return;
                }
                if state.settings.read().close_to_tray {
                    // Recording (if any) continues; the tray shows it.
                    api.prevent_close();
                    let _ = window.hide();
                } else if state.recording_active() {
                    api.prevent_close();
                    tray::request_quit(app);
                } else {
                    api.prevent_close();
                    quit(app);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::set_setting,
            settings::reset_settings,
            media::media_tools,
            media::verify_encoders,
            dashboard::dashboard,
            library::list_media,
            library::list_export_jobs,
            system::frontend_log,
            system::app_paths,
            system::system_info,
            system::read_logs,
            system::diagnostics,
            system::about_info,
            system::open_path,
            system::open_folder,
            system::reveal_path,
            system::open_url,
            system::open_log_folder,
            system::quit_app,
            recording::list_capture_sources,
            recording::recording_status,
            recording::start_video_recording,
            recording::pause_recording,
            recording::resume_recording,
            recording::stop_recording,
            recording::open_region_selector,
            recording::region_selected,
            system::hide_to_tray,
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(|_, _| {}),
        Err(e) => {
            eprintln!("ENRecorder could not start: {e}");
            std::process::exit(1);
        }
    }
}
