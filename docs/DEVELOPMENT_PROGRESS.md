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

## Phase 2, part 2 — Wayland screen recording (2026-11-11)

**Implemented**
* `capture::frames`: frame-stream input for backends that deliver frames themselves — a
  one-frame slot (newest wins, replaced frames counted), a feeder that writes at the recording
  frame rate (repeats still frames, counts late ticks as dropped), raw `bgr0` pipe into FFmpeg.
  Memory is bounded to one frame whatever the length of the recording.
* `capture::wayland`: xdg-desktop-portal ScreenCast via `ashpd` (cursor embedded/hidden as
  chosen, monitor or window, no persistence), PipeWire node read by GStreamer's `pipewiresrc`
  (the Rust PipeWire bindings need PipeWire ≥ 0.3.65; Ubuntu 22.04 ships 0.3.48). Portal
  cancellation is reported as "screen sharing was cancelled".
* Session: frame streams start while preparing, so the countdown and "recording" begin only
  once pictures arrive; parts without any frame (pause/stop right after start) are dropped
  instead of breaking the join.
* App: Wayland sessions use the portal backend; the Video Recorder shows "Choose in the
  sharing dialog". The .deb depends on gstreamer1.0-pipewire and gstreamer1.0-tools.
* New app icon (from the user's design).

**Tests executed**
* `cargo test -p enrec-core`: 55 passed (new: frame slot crop/pad/swap, skipped-frame counting,
  feeder rate, frame-stream recording with real FFmpeg, instant pause).
* `scripts/test/wayland-e2e.sh` in `enrec-build` (Docker): headless sway 1.7 + PipeWire 0.3.48 +
  WirePlumber + xdg-desktop-portal(-wlr 0.5): portal granted 1280×720, recorded 3 s + pause +
  2 s → 5.0 s MP4, 150 frames, 0 dropped, 30 fps.

**Known limitations**
* Not yet tested on GNOME or KDE Wayland on real hardware.
* Windows backend still to come (next).

## Phase 2, part 3 — Windows screen recording (2026-11-11)

**Implemented**
* `capture::windows`: monitors from DXGI (adapter 0 outputs, the numbering `ddagrab` uses),
  windows from `EnumWindows` (visible, not cloaked, not tool windows, own windows excluded,
  executable name as the app).
* Capture through FFmpeg's Windows grabbers so the session (segments, pause/resume,
  finalisation) is shared with Linux: `ddagrab` (Desktop Duplication: GPU copy, physical pixels
  at any display scaling) for monitors, single-monitor desktops and areas on one monitor;
  `gdigrab` for windows (`title=`), multi-monitor desktops and FFmpeg builds without `ddagrab`.
  The FFmpeg build is checked once for `ddagrab` when it is found.

**Tests executed**
* Cross-build for `x86_64-pc-windows-gnu` in `enrec-build`: `cargo clippy -p enrec-core -D
  warnings` clean, `cargo check` of the app clean.

**Known limitations**
* Not run on real Windows yet (no Windows machine here); the CI Windows job builds it.
* A captured window is grabbed from the screen (covering windows are captured too).

## Phase 3 — Audio (2026-11-11)

**Implemented**
* `audio`: device enumeration (`pactl -f json list sources` on Linux — PulseAudio or
  pipewire-pulse, monitors of outputs = system audio, both `monitor_of_sink` and pactl 15.99's
  `monitor_source`; DirectShow on Windows with loopback devices as system audio), FFmpeg inputs
  (`pulse` / `dshow`), the audio plan: per-source volume, `amix` (no normalisation) or separate
  tracks, `aresample=async=1` drift correction, AAC/Opus for video containers, MP3/M4A/Opus/FLAC/WAV
  for audio-only files, and a live peak meter (`astats` through FFmpeg's log: file/pipe outputs are
  buffered until exit).
* Recording session: audio inputs in the same FFmpeg process as the video (one clock, same
  segments, so pause/resume keeps sync), audio-only sessions (verified for audio, `kind = audio`
  rows), meter value in the snapshot, the segment join maps every stream (separate tracks kept).
* App: `list_audio_devices`, `start_audio_recording`; settings for screen-recording audio and the
  Audio Recorder; the Audio Recorder stays visible (no hide, no floating bar).
* UI: Audio section in Recording settings (microphone, system audio, separate tracks), level meter
  and audio info in the live panel, a working Audio Recorder page.

**Tests executed**
* `cargo test -p enrec-core`: 63 passed — new: pactl/dshow parsing, plans (mix, separate, meter),
  levels, formats; real FFmpeg sessions: video + two mixed sources across a pause (meter reads),
  audio-only MP3, separate tracks surviving the join in M4A.
* `scripts/test/audio-e2e.sh` (PipeWire + WirePlumber + pipewire-pulse in `enrec-build`): a virtual
  speaker playing 440 Hz and a virtual microphone (pipe source) playing 880 Hz are listed as system
  audio / microphone (both default); MP3 (mixed) and M4A recordings with pause complete, meter
  -14.5 / -21 dB, files hold the tones.
* `scripts/test/smoke-audio.sh` (Xvfb, UI driven): Audio Recorder → Start → Ctrl+Shift+P twice →
  Ctrl+Shift+R → 4.8 s MP3 in `~/Videos/ENRecorder/Audio`, library row `audio`, session completed.
* Windows cross-build: core clippy clean, app check clean. `cargo clippy -D warnings` (core with
  examples, app): clean. `pnpm typecheck`: clean; `pnpm test`: 9 passed.

**Known limitations**
* Windows system audio needs a loopback device (Stereo Mix / virtual cable).
* Volume sliders are not in the UI yet (the engine supports them; editor phase).

## Phase 4 — Screenshots (2026-11-11)

**Implemented**
* `screenshot`: one-frame grab through the capture backends (`-frames:v 1` PNG), Wayland
  screenshot portal (`ashpd`), PNG header reading, export to PNG/JPEG/WebP via FFmpeg (no image
  codec bundled), originals kept in `.enrecorder-originals`, library rows with the annotation JSON.
* App: `take_screenshot` (delay, hide window, pointer), `screenshot_image`, `save_screenshot`,
  `copy_screenshot` (clipboard plugin, PNG decoded by Tauri), `get_screenshot`.
* UI: Screenshot Studio (full screen / monitor / window / area, delay, recent screenshots) and a
  canvas editor: select/move, arrow, rectangle, ellipse, line, pen, highlight, blur (pixelates the
  original), text, numbered steps, crop, colours, thickness, undo/redo, shortcuts, save, copy.

**Tests executed**
* `cargo test -p enrec-core`: 65 passed (new: PNG header; grab from a synthetic source, export to
  all three formats, refusal of bad input, library insert/update/read).
* `pnpm test`: 14 passed (annotation hit-testing, moving, step numbers, document parsing).
* `scripts/test/smoke-screenshot.sh` (Xvfb, UI driven): Ctrl+4 → Take screenshot → editor opens on
  the 1440×900 capture → rectangle, blur, step → Ctrl+S → PNG saved, original kept, row with
  annotations.
* clippy `-D warnings` (core with Wayland feature, app) and `pnpm typecheck`: clean.

**Known limitations**
* Window screenshots on X11/Windows capture what is visible on screen.
* No live area selection on screen yet: the area is cropped from a full capture (frozen screen).
