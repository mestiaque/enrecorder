# ENRecorder — Features and status

Status words: **Done** (built and tested as described), **Partial**, **Planned (phase N)**,
**Not tested** (built but not verified on that platform). Nothing is listed as done on the
strength of mocked tests alone.

## Foundation (phase 1)

| Feature | Status | Notes |
|---|---|---|
| Tauri 2 shell, frameless window, EN* window controls | Done | Ubuntu tested (Xvfb + real desktop) |
| Sidebar navigation (7 pages), active state, collapsed mode | Done | Ctrl+1…7, Ctrl+B |
| Themes Dark / Light / Glass / System, accent colour, reduce motion | Done | persisted |
| Home dashboard: recent media, statistics, storage, disk space | Done | data from SQLite + real folder walk |
| System readiness (display server, desktop, PipeWire/Pulse, FFmpeg, FFprobe, encoders, disk) | Done | |
| Display server detection X11 / Wayland / Windows | Done | unit tested with fake environments; X11 verified on hardware, Wayland not yet on hardware |
| Monitor list (name, resolution, position, scale, primary) | Done | from Tauri |
| FFmpeg/FFprobe discovery (custom path → bundled → PATH) and version | Done | |
| Encoder verification by test encode, cached per FFmpeg path+version | Done | H.264/HEVC/AV1/VP9: software, NVENC, AMF, QSV, VA-API, Media Foundation |
| SQLite with versioned migrations, UUID keys, foreign keys, WAL | Done | 11 tables (brief §13) |
| Startup recovery marking (interrupted sessions/exports) | Done | container repair: planned (phase 7) |
| Settings: typed, validated per key, persisted, reset | Done | folders are checked for write access before saving |
| Recordings library list: filter, search, sort | Done | empty until capture exists |
| Export Center list | Done | empty until export exists |
| Tray (open, quit), close to tray, start minimized, start with system | Done | |
| Logs viewer, copy diagnostics | Done | |
| Windows build | Not tested | code is `cfg`-gated; no Windows machine in this environment |

## Screen recording (phase 2)

| Feature | Status | Notes |
|---|---|---|
| X11 backend: entire desktop, monitor (RandR), window (EWMH list), area | Done | FFmpeg `x11grab` (XCB/XShm); tested in Xvfb (UI end to end) and on real hardware (Intel UHD 620, GNOME X11) |
| Area selection overlay (drag, size label, Enter/Esc) | Done | transparent over GNOME; opaque without a compositor |
| Recording settings: 24/30/60/120 fps, native/720p/1080p/1440p/4K (downscale only), MP4/MKV/WebM, H.264/HEVC/AV1/VP9, encoder (automatic = best verified, hardware first), quality, cursor | Done | invalid combinations refused before recording |
| Countdown (0/3/5/10 s, cancellable), pause/resume, stop, timer, auto stop after N minutes | Done | pause = new MKV segment; segments joined without re-encoding |
| Recording state machine wired to the session (idle → preparing → countdown → recording ⇄ paused → finalizing → completed/failed/cancelled) | Done | every state stored in `capture_sessions` |
| Output verification with FFprobe, library row + metadata, automatic naming, unique file names | Done | |
| Disk space monitoring (stops cleanly below 500 MB free) | Done | |
| Failure handling: FFmpeg that does not start / dies → failed with its message; parts kept for recovery | Done | |
| Floating controls (timer, pause/resume, stop), hide main window while recording, red tray icon + tooltip, tray start/stop/pause | Done | on X11 the floating bar is recorded when it is inside the recorded area |
| Global shortcuts Ctrl+Shift+R (start/stop), Ctrl+Shift+P (pause/resume) | Done | fixed for now; customisable shortcuts later |
| Quit while recording asks first, then stops and saves | Done | |
| X11 window capture limits | Known | covered windows are captured too; a window keeps its starting size |
| Wayland backend (xdg-desktop-portal ScreenCast → PipeWire via GStreamer `pipewiresrc` → frame pipe → FFmpeg) | Done | tested end to end on headless sway + xdg-desktop-portal-wlr (Docker); **not yet tested on GNOME/KDE Wayland hardware**. Portal cancel = recording cancelled with a message |
| Frame-stream pipeline: one-frame slot, fixed-rate feeder, still frames repeated, late ticks counted as dropped | Done | used by Wayland |
| Windows backend: monitors/areas with FFmpeg `ddagrab` (Desktop Duplication, physical pixels at any scaling), windows and multi-monitor desktop with `gdigrab`; monitors from DXGI, windows from EnumWindows | Done | cross-built and clippy-clean; **not yet run on Windows hardware**; window capture records what is visible |
| Scheduled recording, file splitting | Planned (phase 6) | |
| Audio in screen recordings: microphone and/or system audio, mixed or on separate tracks, AAC (MP4/MKV) or Opus (WebM), same FFmpeg process and segments as the video (one clock) | Done | |

## Audio (phase 3)

| Feature | Status | Notes |
|---|---|---|
| Device list: microphones and system audio (monitors of outputs) via `pactl` (PulseAudio or PipeWire's pipewire-pulse); defaults marked | Done | without `pactl` the server defaults are offered |
| Windows devices via DirectShow (`ffmpeg -list_devices`); loopback devices (Stereo Mix, virtual cables) offered as system audio | Done | WASAPI loopback without such a device is not available through FFmpeg — known limitation |
| Audio Recorder page: microphone, system audio or both; MP3, M4A (AAC), Opus, FLAC, WAV; quality; pause/resume; saved in `<recordings>/Audio` and the library | Done | |
| Mixer: per-source volume, `amix` without normalisation, `aresample=async` drift correction | Done | volume sliders in the UI come with the editor (phase 5) |
| Multi-track: separate tracks in MP4/MKV/WebM screen recordings and M4A audio; one-track formats (MP3/WAV/FLAC/Opus) mix | Done | the segment join keeps every stream |
| Live peak meter (0.1 s) in the recorder and the live panel | Done | via FFmpeg `astats` |

## Screenshots (phase 4)

| Feature | Status | Notes |
|---|---|---|
| Capture: full screen, monitor, window (X11/Windows, one frame through the same FFmpeg backends), area (captured, then cropped in the editor) | Done | |
| Wayland: the desktop's screenshot portal (its own screen/window/area picker) | Done | needs xdg-desktop-portal with a screenshot backend (GNOME, KDE) |
| Delay (3/5/10 s), hide ENRecorder while capturing, include the pointer | Done | |
| Editor: select/move, arrow, rectangle, ellipse, line, pen, highlight, blur (pixelate the original), text, numbered steps, crop; 8 colours, 3 thicknesses; undo/redo; keyboard shortcuts | Done | |
| Save as PNG, JPEG or WebP (FFmpeg encodes JPEG/WebP); copy to the clipboard | Done | |
| Original kept, annotations stored as JSON: a saved screenshot can be edited again | Done | |

## Planned

| Area | Phase |
|---|---|
| Library actions (rename, delete, tags, favourites, preview, metadata via FFprobe, thumbnails); export queue; basic editor | 5 |
| Webcam overlay, cursor effects, keystroke overlay, audio processing, OCR, optional transcription | 6 |
| Recovery (MKV finalisation), performance, security review, installers (Windows NSIS/portable, AppImage), FFmpeg bundling for Windows | 7 |
