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
| Recordings library list: filter, search, sort | Done | |
| Export Center list | Done | |
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
| Floating controls (timer, pause/resume, stop, sound level with clipping/silence hints), hide main window while recording, red tray icon + tooltip, tray start/stop/pause | Done | on X11 the floating bar is recorded when it is inside the recorded area |
| Global shortcuts: start/stop (Ctrl+Shift+R), pause/resume (Ctrl+Shift+P), screenshot (Ctrl+Shift+S); changed or turned off in Settings → Shortcuts, stored in SQLite | Done | a combination used by another action or held by another program is refused (the old one stays) |
| Quit while recording asks first, then stops and saves | Done | |
| X11 window capture limits | Known | covered windows are captured too; a window keeps its starting size |
| Wayland backend (xdg-desktop-portal ScreenCast → PipeWire via GStreamer `pipewiresrc` → frame pipe → FFmpeg) | Done | tested end to end on headless sway + xdg-desktop-portal-wlr (Docker); **not yet tested on GNOME/KDE Wayland hardware**. Portal cancel = recording cancelled with a message |
| Frame-stream pipeline: one-frame slot, fixed-rate feeder, still frames repeated, late ticks counted as dropped | Done | used by Wayland |
| Windows backend: monitors/areas with FFmpeg `ddagrab` (Desktop Duplication, physical pixels at any scaling), windows and multi-monitor desktop with `gdigrab`; monitors from DXGI, windows from EnumWindows | Done | cross-built and clippy-clean; **not yet run on Windows hardware**; window capture records what is visible |
| Scheduled recording: date and time, one at a time, banner on the Video Recorder page and time in the status bar, cancel; a desktop notification if it cannot start | Done | the app must be running (tray is enough) |
| File splitting: a new complete file every 5–120 minutes, "<name> (part N)", each verified and in the library | Done | |
| Camera as the only source (Webcam tab in the source picker) | Done | |
| Software-only encoding (Settings → Performance) | Done | for drivers whose hardware encoders misbehave |
| Audio in screen recordings: microphone and/or system audio, mixed or on separate tracks, AAC (MP4/MKV) or Opus (WebM), same FFmpeg process and segments as the video (one clock) | Done | |

## Audio (phase 3)

| Feature | Status | Notes |
|---|---|---|
| Device list: microphones and system audio (monitors of outputs) via `pactl` (PulseAudio or PipeWire's pipewire-pulse); defaults marked | Done | without `pactl` the server defaults are offered |
| Windows devices via DirectShow (`ffmpeg -list_devices`); loopback devices (Stereo Mix, virtual cables) offered as system audio | Done | WASAPI loopback without such a device is not available through FFmpeg — known limitation |
| Audio Recorder page: microphone, system audio or both; MP3, M4A (AAC), Opus, FLAC, WAV; quality; pause/resume; saved in `<recordings>/Audio` and the library | Done | |
| Mixer: per-source volume (sliders in the recorder), `amix` without normalisation, `aresample=async` drift correction | Done | |
| Sample rate (44.1/48 kHz) and mono/stereo; OGG (Vorbis) in the Audio Recorder | Done | |
| Microphone noise gate (`agate`) | Done | |
| Echo cancellation: PulseAudio/PipeWire `module-echo-cancel` loaded for the recording and unloaded afterwards | Done | Linux only; needs `pactl`; on Windows the option is hidden |
| Multi-track: separate tracks in MP4/MKV/WebM screen recordings and M4A audio; one-track formats (MP3/WAV/FLAC/Opus) mix | Done | the segment join keeps every stream |
| Live meter: peak and RMS (0.1 s), level history, clipping count, warning after 10 s of silence | Done | via FFmpeg `astats` |

## Screenshots (phase 4)

| Feature | Status | Notes |
|---|---|---|
| Capture: full screen, monitor, window (X11/Windows, one frame through the same FFmpeg backends), area (captured, then cropped in the editor) | Done | |
| Wayland: the desktop's screenshot portal (its own screen/window/area picker) | Done | needs xdg-desktop-portal with a screenshot backend (GNOME, KDE) |
| Delay (3/5/10 s), hide ENRecorder while capturing, include the pointer | Done | |
| **Quick capture** (Flameshot/ShareX style): Ctrl+Alt+A (changeable), the tray or the Screenshot Studio button freezes the whole desktop; drag an area (magnifier, size label), move/resize it with handles, draw in place (arrow, rectangle, ellipse, line, pen, highlight, blur, text, numbered steps; 6 colours, 3 sizes, undo/redo, tool keys), then Enter/Ctrl+C copies, Ctrl+S saves to the library (and copies), "Studio" opens the full editor, Esc cancels. The full capture is kept as the original, so a saved quick capture can be re-cropped and re-edited | Done | X11 tested end to end in Xvfb (save + copy checked); multi-monitor X11/Windows: the overlay spans every monitor; Wayland: the desktop may ask once for screenshot permission and the overlay covers the current monitor (not tested on GNOME/KDE hardware) |
| Editor: select/move, arrow, rectangle, ellipse, line, pen, highlight, blur (pixelate the original), text, numbered steps, crop; 8 colours + any colour, 3 thicknesses; undo/redo; bring forward/send back; copy/paste annotations; zoom (fit, 50–200 %); keyboard shortcuts | Done | |
| Finish: rotate, scale, border (colour), shadow — applied when saving or copying | Done | |
| Save as PNG, JPEG or WebP (FFmpeg encodes JPEG/WebP); copy to the clipboard; default format/quality and a file name template ({date}, {time}, {mode}) in Settings → Screenshots | Done | |
| Scrolling capture | Not offered | no reliable, content-aware way to scroll arbitrary windows on X11, Wayland and Windows; the brief allows leaving it out rather than faking it |
| Original kept, annotations stored as JSON: a saved screenshot can be edited again | Done | |

## Library, export and editor (phase 5)

| Feature | Status | Notes |
|---|---|---|
| Recordings: thumbnails (video frame, audio waveform, scaled screenshot; cached), detail panel with player/preview, FFprobe facts, multi-track count | Done | |
| Rename (file and entry), move to the system trash, favourites, show in folder, open with the default app | Done | |
| Tags (create, assign, delete), filters by date, length, size, resolution and tag; list or grid view; select several to tag or delete together | Done | |
| Missing-file check (moved/deleted files marked), "Re-read file" (FFprobe + new thumbnail), save the current video frame as a screenshot | Done | |
| Preview server: recordings stream over HTTP on 127.0.0.1 (random port + per-run token, library items only, Range/seek) | Done | WebKitGTK cannot play `asset:` URLs |
| Export queue: convert (MP4/WebM/MKV), compress (smaller CRF + size), audio only (MP3/M4A/WAV/FLAC/Opus), GIF (palette), trim; one job at a time, progress, cancel, retry, the same export twice refused, persisted, interrupted jobs marked failed on start, results verified and added to the library | Done | |
| Export options: crop, rotate, speed 0.25–4× (video and sound), fade in/out, frame rate, codec, video/audio bitrate, size estimate | Done | |
| Media Editor: preview, start/end (sliders or "at the current position"), play selection, volume, remove sound, crop by dragging on the video, rotate, size, speed, fades, split at the playhead (two exports), save frame, find silences (and trim leading/trailing silence), waveform under the timeline, output format, non-destructive export | Done | a multi-clip timeline is not built |

## Advanced (phase 6)

| Feature | Status | Notes |
|---|---|---|
| Webcam overlay in screen recordings: camera list (Linux sysfs / v4l2, Windows DirectShow), corner, size 15–40 %, circle, rectangle or rounded, optional frame colour and shadow; overlaid before the GPU upload so hardware encoders still work | Done | every shape/frame/shadow combination runs in real FFmpeg in the tests (test pattern as the camera) |
| Microphone clean-up while recording: high-pass + FFT denoiser, real-time dynamic normalisation (Video and Audio Recorder) | Done | |
| Export clean-up: denoise and loudness normalisation to −16 LUFS (Media Editor) | Done | |
| OCR: "Text" in the screenshot editor reads the current image (crop applied, blurred parts unreadable) with Tesseract, copy to the clipboard | Done | needs `tesseract-ocr` (Ubuntu) or the UB Mannheim build (Windows); English and Bengali used when installed |
| Click highlights and keystroke overlay: with "Remember clicks" / "Remember keys" the pointer, clicks and key combinations are sampled during the recording (X11 `QueryPointer`/`QueryKeymap`, Windows `GetCursorPos`/`GetAsyncKeyState`, 16 ms) into a track file; Export can burn them in (ASS subtitles via libass), following trim, crop, speed and scale | Done | X11 tested end to end (clicks and Ctrl/Alt combinations recorded from a real recording); Windows cross-built only; **not available on Wayland** (no global pointer/keyboard access by design). "Remember key presses" is off by default and says that it records what is typed (kept on this computer only); modifiers alone are not shown |
| Transcription (optional): "Transcribe" in a recording's details writes "<name>.srt" next to it with whisper.cpp (`whisper-cli` + a `ggml-*.bin` model, found automatically or chosen in Settings → Tools), language Detect/English/Bengali/…, progress and cancel | Done | whisper.cpp is **not bundled**; the app says so when it is missing. Tested with real FFmpeg and a stand-in `whisper-cli` (flags checked against whisper.cpp 1.7.6); not run with the real engine in this environment |

## Production (phase 7)

| Feature | Status | Notes |
|---|---|---|
| Recovery of interrupted recordings: Home lists sessions whose parts were left by a crash or power loss; **Recover** remuxes every readable part (corrupt tails dropped) and joins them into "<title> (recovered).mkv" in the library; **Discard** deletes the parts | Done | tested with a recording killed mid-way |
| In-app updates from GitHub releases: check (at most daily, can be turned off), SHA-256 verified download, install in place (.deb via pkexec, AppImage swap, silent NSIS), restart | Done | refuses while recording |
| Installers: Ubuntu .deb and AppImage, Windows NSIS (current user) with FFmpeg + FFprobe bundled | Done | built by CI on a `v*` tag |
| Security review: argument-array processes, CSP, minimal capabilities, asset scope limited to ENRecorder's folders, loopback preview server with token, `open_path` limited to folders and media | Done | [SECURITY.md](SECURITY.md) |
| Desktop notifications when a recording, export or transcript is saved, or a recording (also a scheduled one) fails; can be turned off | Done | screenshots show an in-app toast instead |
| Device manager (Settings → Devices): displays, microphones, system audio, cameras with connected/unplugged state, kept in the `devices` table | Done | |
| Privacy: log retention (old log files deleted at start), notifications on/off; thumbnails on/off (Performance) | Done | |
| File associations | Not registered | ENRecorder is a recorder, not a player; media files keep opening in the user's player |

## Not tested on hardware

| Platform | State |
|---|---|
| Windows 10/11 | cross-compiled and clippy-clean (`x86_64-pc-windows-gnu`); installers built by CI; not run on a Windows machine |
| Wayland on GNOME / KDE | tested on headless sway + xdg-desktop-portal-wlr only |
