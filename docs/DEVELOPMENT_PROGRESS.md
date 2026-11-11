# Development progress

## Phase 1 — Foundation (2026-11-11)

**Implemented**
* Workspace: `crates/enrec-core` (no Tauri) + `src-tauri` + React frontend; family UI components.
* SQLite: versioned migrations (`schema_migrations`), all 11 tables from the brief with UUID keys,
  foreign keys, indexes, `CHECK`s; WAL; per-transaction migrations; refusal of newer schemas;
  startup recovery marking of interrupted sessions/exports.
* Settings: typed, validated per key, stored per key; folder checks (absolute, creatable,
  writable) before saving; reset.
* FFmpeg service: discovery, version parsing, encoder listing, verification by test encode with
  hardware setup (VA-API/QSV), time limits, skipping a hung device's other encoders, cache.
* System detection: OS, display server (X11/Wayland/Windows), desktop, PipeWire/Pulse sockets,
  render nodes; storage usage and free space.
* Capture interfaces and the recording state machine (no backends yet).
* UI: sidebar, toolbar, status bar, Home dashboard, Video/Audio/Screenshot/Editor pages with
  honest "phase N" notices and real detected data, Recordings and Export Center lists, Settings
  (General, Storage, Tools, Appearance, Logs, About), tray, shortcuts, themes.
* Docs: README, ARCHITECTURE, FEATURES, API, BUILD, TROUBLESHOOTING, this file.

**Tests executed**
* `cargo test -p enrec-core`: 32 passed (includes real FFmpeg test encodes on this machine).
* `cargo clippy -p enrec-core --all-targets -D warnings`: clean.
* `pnpm typecheck` (strict): clean. `pnpm test` (Vitest): 9 passed.
* `pnpm tauri build --bundles deb` in Docker: builds without warnings.
* `scripts/test/smoke-ui.sh` (Xvfb): all 7 pages and the settings sections render; theme change
  persisted to SQLite; 12 tables + migration 1 present; integrity check ok.

**Known limitations**
* No recording yet (phases 2–4).
* Windows not built or tested; Wayland not tested on hardware (detection is unit-tested only).
* On this test machine (Intel UHD 620, FFmpeg 4.4) QSV test encodes hang and are skipped; on the
  Docker build image no GPU is present, so only software encoders verify there.

**Next:** phase 2 — capture foundation (X11 backend first on this machine, then Wayland portal +
PipeWire, then Windows.Graphics.Capture), FFmpeg recording pipeline, recording HUD.

## Phase 2, part 1 — X11 screen recording (2026-11-11)

**Implemented**
* `capture::x11`: sources from the X server (RandR monitors, EWMH windows with titles and
  apps, own windows excluded), FFmpeg `x11grab` input for desktop/monitor/window/area, clamped
  and even sizes.
* `recording`: options, validation (format ↔ codec, fps), encoder choice (verified only,
  hardware first), per-encoder quality settings, downscale-only resolution, unique names.
* `recording::session`: thread per recording driving the state machine; MKV segments
  (pause/resume), graceful stop (`q`), kill after 10 s, early-failure detection with FFmpeg's
  message, progress from `-progress`, disk watch, auto stop, join without re-encoding, FFprobe
  verification, library + metadata rows, parts kept on failure.
* `media::probe`: FFprobe JSON → container, codecs, size, fps, duration, bitrate, streams.
* App: recording commands and events, floating controls window, area overlay window, tray
  recording state (red icon, tooltip, start/stop/pause items), global shortcuts, hide window
  while recording, quit-while-recording confirmation, recording defaults in Settings.
* UI: Video Recorder page (source picker with screen/window/area, settings form, live panel,
  countdown overlay), sidebar/status bar recording indicators, "Recording saved" toast.

**Tests executed**
* `cargo test -p enrec-core`: 49 passed, including real FFmpeg sessions (record → pause →
  resume → stop → 2 segments joined → FFprobe-verified MP4; cancel during countdown; broken
  capture fails with FFmpeg's message) and the real X server source list.
* `cargo clippy -p enrec-core --all-targets --examples -D warnings`: clean. `pnpm typecheck`:
  clean. `pnpm test`: 9 passed. Release .deb builds without warnings.
* Real hardware (this machine, X11/GNOME, Intel UHD 620): `examples/record_desktop` recorded
  1920×1080 with `h264_vaapi` and `libx264`, one pause each: 5.0 s files, ~30 fps, 0 dropped.
* `scripts/test/smoke-record.sh` (Xvfb, UI driven): Start button → countdown → recording →
  floating controls pause/resume/stop → 5.1 s MP4 from 2 segments; Ctrl+Shift+R start/stop →
  second recording; library rows, metadata and completed sessions in SQLite.
* `scripts/test/smoke-region.sh`: area drawn on the overlay → 400×300 recording at 300,300.

**Known limitations**
* Wayland and Windows backends are not built yet (next).
* No audio in recordings (phase 3). Shortcuts are fixed (customisable later).
* X11 window capture records whatever covers the window; resizing is not followed.
