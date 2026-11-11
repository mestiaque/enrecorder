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

## Planned

| Area | Phase |
|---|---|
| Screen capture: Windows.Graphics.Capture, X11, Wayland portal + PipeWire; recording state machine wired to FFmpeg; HUD; countdown; pause/resume; file splitting; disk monitoring | 2 |
| Audio: device manager, microphone, system audio (WASAPI loopback / PipeWire), meters, multi-track, mixing | 3 |
| Screenshots: full screen, monitor, window, region, delay; annotation editor; PNG/JPEG/WebP; clipboard | 4 |
| Library actions (rename, delete, tags, favourites, preview, metadata via FFprobe, thumbnails); export queue; basic editor | 5 |
| Webcam overlay, cursor effects, keystroke overlay, audio processing, OCR, optional transcription | 6 |
| Recovery (MKV finalisation), performance, security review, installers (Windows NSIS/portable, AppImage), FFmpeg bundling for Windows | 7 |
