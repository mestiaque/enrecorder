//! Tray icon: open, start/stop recording, pause/resume, quit. While recording the icon turns
//! into a red dot and the tooltip says so, so a hidden window never means hidden recording.

use enrec_core::capture::state::RecState;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

/// Menu items whose text changes with the recording state.
pub struct TrayMenu {
    record: MenuItem<Wry>,
    pause: MenuItem<Wry>,
}

pub const EV_CONFIRM_QUIT: &str = "confirm-quit";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open ENRecorder", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Start recording", true, Some("Ctrl+Shift+R"))?;
    let pause = MenuItem::with_id(app, "pause", "Pause", false, Some("Ctrl+Shift+P"))?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(app, &[&open, &sep()?, &record, &pause, &sep()?, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("ENRecorder")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "record" => {
                show_main_if_idle(app);
                crate::commands::recording::toggle_record(app);
            }
            "pause" => crate::commands::recording::toggle_pause(app),
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    app.manage(TrayMenu { record, pause });
    Ok(())
}

fn show_main_if_idle(app: &AppHandle) {
    if !app.state::<crate::state::AppState>().recording_active() {
        show_main(app);
    }
}

/// Quit, but ask first while a recording is running (the main window shows the question).
pub fn request_quit(app: &AppHandle) {
    if app.state::<crate::state::AppState>().recording_active() {
        show_main(app);
        let _ = app.emit_to("main", EV_CONFIRM_QUIT, ());
    } else {
        crate::quit(app);
    }
}

/// Updates the tray for a recording state.
pub fn show_recording(app: &AppHandle, st: RecState) {
    let active = st.is_active();
    if let Some(m) = app.try_state::<TrayMenu>() {
        let _ = m.record.set_text(if active { "Stop recording" } else { "Start recording" });
        let _ = m.pause.set_text(if st == RecState::Paused { "Resume" } else { "Pause" });
        let _ = m.pause.set_enabled(matches!(st, RecState::Recording | RecState::Paused));
    }
    if let Some(tray) = app.tray_by_id("main") {
        let icon = if active { Image::from_bytes(include_bytes!("../icons/tray-recording.png")).ok() } else { app.default_window_icon().cloned() };
        if let Some(i) = icon {
            let _ = tray.set_icon(Some(i));
        }
        let tip = match st {
            RecState::Countdown | RecState::Preparing => "ENRecorder — starting…",
            RecState::Recording => "ENRecorder — recording",
            RecState::Paused => "ENRecorder — paused",
            RecState::Finalizing => "ENRecorder — saving the recording…",
            _ => "ENRecorder",
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

/// Shrink the window to fit small screens so the header stays visible.
pub fn fit_to_monitor(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    // A hidden window may report a 0×0 monitor; fall back to the primary one.
    let usable = |m: &tauri::Monitor| m.size().width >= 320 && m.size().height >= 240;
    let monitor = w.current_monitor().ok().flatten().filter(usable).or_else(|| w.primary_monitor().ok().flatten().filter(usable));
    let Some(m) = monitor else { return };
    let scale = m.scale_factor();
    let screen = m.size().to_logical::<f64>(scale);
    // A hidden GTK window reports a bogus size, so use the configured one.
    let Some(cfg) = app.config().app.windows.iter().find(|c| c.label == "main") else { return };
    let (max_w, max_h) = (screen.width * 0.94, screen.height * 0.92);
    if cfg.width > max_w || cfg.height > max_h {
        let size = tauri::LogicalSize::new(cfg.width.min(max_w), cfg.height.min(max_h));
        let _ = w.set_size(size);
        let origin = m.position().to_logical::<f64>(scale);
        let _ = w.set_position(tauri::LogicalPosition::new(origin.x + (screen.width - size.width) / 2.0, origin.y + (screen.height - size.height) / 2.0));
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
