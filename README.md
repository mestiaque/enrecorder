# ENRecorder

**One studio. Every capture.** Screen recording, audio recording and screenshots for Windows
10/11 and Ubuntu 22.04/24.04 (X11 and Wayland), with a media library, a simple editor and an
export queue. Local-first: no account, no cloud upload, no watermark, no telemetry.

ENRecorder is part of the EN* family (ENViewer, ENPlayer, ENSave, ENStream, ENiShare, ENPost)
and uses the same look, themes (Dark, Light, Glass, System) and settings layout.

> **Status: phase 7 done (all planned phases).** Screen recording on **Ubuntu X11**, **Wayland** (sharing dialog,
> portal + PipeWire) and **Windows** (Desktop Duplication / GDI through FFmpeg), with microphone
> and system audio (mixed or separate tracks), pause/resume, floating controls, tray and
> shortcuts; an **Audio Recorder** with live meter; a **Screenshot Studio** with an annotation
> editor (arrows, shapes, text, steps, blur, crop) and PNG/JPEG/WebP export; a **library** with
> thumbnails, preview, rename/trash/favourites; an **Export Center** (convert, compress, audio,
> GIF, trim) and a **Media Editor** for trimming; a **webcam overlay**, microphone **noise
> reduction / levelling**, loudness normalisation on export and **OCR** of screenshots
> (Tesseract); **recovery** of interrupted recordings, **in-app updates** and installers for
> Ubuntu (.deb, AppImage) and Windows (NSIS with FFmpeg bundled). See [FEATURES.md](docs/FEATURES.md) and
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

## Install

**Ubuntu 22.04 / 24.04**

```bash
sudo apt install ./ENRecorder_0.2.0_amd64.deb   # pulls in ffmpeg and the PipeWire GStreamer plugin
```

or the AppImage (needs `ffmpeg` installed: `sudo apt install ffmpeg`):

```bash
chmod +x ENRecorder_0.2.0_amd64.AppImage && ./ENRecorder_0.2.0_amd64.AppImage
```

OCR in the screenshot editor needs `sudo apt install tesseract-ocr` (add `tesseract-ocr-ben`
for Bengali).

**Windows 10 / 11**: run `ENRecorder_0.2.0_x64-setup.exe`. It installs for the current user (no
administrator rights) and includes FFmpeg; WebView2 is downloaded if it is missing.

New versions are offered inside the app (Settings → Updates), checked against the release's
SHA-256 before they are installed.

**Uninstall**: `sudo apt remove enrecorder` (Ubuntu .deb), delete the AppImage, or *Settings →
Apps → ENRecorder → Uninstall* on Windows. Your recordings stay in your Videos/Pictures folders;
settings and the library are in `~/.local/share/com.mestiaque.enrecorder`
(`%APPDATA%\com.mestiaque.enrecorder` on Windows).

Build instructions: [docs/BUILD.md](docs/BUILD.md).

## Documentation

| File | Contents |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | processes, services, capture abstraction, data model |
| [docs/FEATURES.md](docs/FEATURES.md) | every feature with its real status |
| [docs/API.md](docs/API.md) | Tauri commands and events with their types |
| [docs/BUILD.md](docs/BUILD.md) | Ubuntu and Windows build instructions, dependencies |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | common problems |
| [docs/SECURITY.md](docs/SECURITY.md) | network use, processes, preview server, files |
| [docs/DEVELOPMENT_PROGRESS.md](docs/DEVELOPMENT_PROGRESS.md) | milestone reports |

## Licence

© 2026 M. Estiaque Ahmed Khan · MIT. FFmpeg is a separate program under its own licence
(LGPL/GPL); on Ubuntu it is the distribution package, on Windows a static build from
[BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) installed next to ENRecorder with its licence
(`FFMPEG-LICENSE.txt`).
