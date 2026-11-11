# ENRecorder

**One studio. Every capture.** Screen recording, audio recording and screenshots for Windows
10/11 and Ubuntu 22.04/24.04 (X11 and Wayland), with a media library, a simple editor and an
export queue. Local-first: no account, no cloud upload, no watermark, no telemetry.

ENRecorder is part of the EN* family (ENViewer, ENPlayer, ENSave, ENStream, ENiShare, ENPost)
and uses the same look, themes (Dark, Light, Glass, System) and settings layout.

> **Status: phase 5 done.** Screen recording on **Ubuntu X11**, **Wayland** (sharing dialog,
> portal + PipeWire) and **Windows** (Desktop Duplication / GDI through FFmpeg), with microphone
> and system audio (mixed or separate tracks), pause/resume, floating controls, tray and
> shortcuts; an **Audio Recorder** with live meter; a **Screenshot Studio** with an annotation
> editor (arrows, shapes, text, steps, blur, crop) and PNG/JPEG/WebP export; a **library** with
> thumbnails, preview, rename/trash/favourites; an **Export Center** (convert, compress, audio,
> GIF, trim) and a **Media Editor** for trimming. Advanced features (webcam, cursor effects, OCR)
> and production polish come next. See [FEATURES.md](docs/FEATURES.md) and
> [DEVELOPMENT_PROGRESS.md](docs/DEVELOPMENT_PROGRESS.md).

## What works today

* Home dashboard: recent recordings and screenshots, statistics, storage use, free disk space
  and a system readiness check (display server, audio server, FFmpeg, FFprobe, encoders, disk).
* Video Recorder (X11): record the desktop, a monitor, a window or an area you draw; 24–120 fps,
  MP4/MKV/WebM, H.264/HEVC/AV1/VP9 with the best **verified** encoder (GPU first); countdown,
  pause/resume, auto stop, floating controls, red tray icon, Ctrl+Shift+R / Ctrl+Shift+P.
* Recordings library and Export Center (read side; they fill up once capture and export exist).
* Settings: start with the system, start minimized, close to tray, save folders (checked before
  saving), a specific FFmpeg, theme, accent colour, animations, logs and diagnostics, About.
* Tray icon, single instance, keyboard shortcuts (Ctrl+1…7 pages, Ctrl+B sidebar, Ctrl+, settings).

## Install (Ubuntu)

```bash
sudo apt install ./ENRecorder_0.2.0_amd64.deb   # pulls in ffmpeg
```

Build instructions: [docs/BUILD.md](docs/BUILD.md).

## Documentation

| File | Contents |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | processes, services, capture abstraction, data model |
| [docs/FEATURES.md](docs/FEATURES.md) | every feature with its real status |
| [docs/API.md](docs/API.md) | Tauri commands and events with their types |
| [docs/BUILD.md](docs/BUILD.md) | Ubuntu and Windows build instructions, dependencies |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | common problems |
| [docs/DEVELOPMENT_PROGRESS.md](docs/DEVELOPMENT_PROGRESS.md) | milestone reports |

## Licence

© 2026 M. Estiaque Ahmed Khan · MIT. FFmpeg is a separate program under its own licence
(LGPL/GPL); on Ubuntu it is the distribution package.
