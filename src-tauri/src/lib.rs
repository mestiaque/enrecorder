mod commands;
mod state;
mod tray;

use commands::{dashboard, library, media, settings, system};
use enrec_core::database::Db;
use enrec_core::settings::Settings;
use parking_lot::RwLock;
use state::{AppState, MediaTools};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, WindowEvent};

/// Quit for real (tray → Quit, menu → Quit): lets the window close instead of hiding it.
pub fn quit(app: &tauri::AppHandle) {
    app.state::<AppState>().quitting.store(true, Ordering::SeqCst);
    log::info!("quitting");
    app.exit(0);
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
    Ok(AppState {
        db: std::sync::Arc::new(db),
        settings: RwLock::new(settings),
        system: RwLock::new(enrec_core::system::detect()),
        tools: RwLock::new(MediaTools::default()),
        quitting: AtomicBool::new(false),
        data_dir,
        log_dir,
        system_videos,
        system_pictures,
    })
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
        .setup(|app| {
            let state = build_state(app.handle())?;
            let hidden = std::env::args().any(|a| a == "--minimized") || state.settings.read().start_minimized;
            let s = state.system.read().clone();
            log::info!("ENRecorder {} on {} · display {:?} · desktop {}", app.package_info().version, s.os_name, s.display_server, s.desktop);
            app.manage(state);
            media::refresh_tools(app.handle().clone(), false);
            tray::install(app.handle())?;
            tray::fit_to_monitor(app.handle());
            if !hidden {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                if !state.quitting.load(Ordering::SeqCst) && state.settings.read().close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
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
