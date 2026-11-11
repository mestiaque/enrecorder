# ENRecorder

**One studio. Every capture.** Screen recording, audio recording and screenshots for Windows
10/11 and Ubuntu 22.04/24.04 (X11 and Wayland), with a media library, a simple editor and an
export queue. Local-first: no account, no cloud upload, no watermark, no telemetry.

ENRecorder is part of the EN* family (ENViewer, ENPlayer, ENSave, ENStream, ENiShare, ENPost)
and uses the same look, themes (Dark, Light, Glass, System) and settings layout.

> **Status: 1.0 — every part of the brief is built.** Screen recording on **Ubuntu X11**,
> **Wayland** (sharing dialog, portal + PipeWire) and **Windows** (Desktop Duplication / GDI
> through FFmpeg). What is tested where, and the few things deliberately left out, are listed in
> [FEATURES.md](docs/FEATURES.md); milestone reports are in
> [DEVELOPMENT_PROGRESS.md](docs/DEVELOPMENT_PROGRESS.md).

## What works today

* **Video Recorder**: desktop, monitor, window, area or camera; 24–120 fps, MP4/MKV/WebM,
  H.264/HEVC/AV1/VP9 with the best **verified** encoder (GPU first, or software only);
  countdown, pause/resume, auto stop, file splitting, scheduled start; microphone and system
  audio (mixed or separate tracks, volumes, noise reduction, noise gate, echo cancellation);
  webcam overlay (circle, rectangle, rounded, frame, shadow); floating controls with a sound
  level; optional click and key tracking for highlights in exports (X11, Windows).
* **Audio Recorder**: MP3, M4A, Opus, OGG, FLAC, WAV; 44.1/48 kHz, mono/stereo; peak + RMS meter
  with level history, clipping and silence warnings.
* **Screenshot Studio**: screen, monitor, window or area with a delay; annotation editor
  (arrows, shapes, text, steps, blur, crop, any colour, layers, zoom, copy/paste), Finish
  (rotate, scale, border, shadow), OCR (Tesseract), PNG/JPEG/WebP.
* **Library**: thumbnails, preview, tags, filters, grid/list, multi-select, favourites,
  missing-file check, save a video frame, optional **transcription** to .srt (whisper.cpp).
* **Media Editor and Export Center**: trim, split, crop, rotate, speed, fades, volume, find and
  trim silences, convert/compress/audio/GIF, codec/frame rate/bitrate with a size estimate, a
  queue with progress, cancel and retry.
* Recovery of interrupted recordings, in-app updates, tray, configurable global shortcuts
  (Ctrl+Shift+R / P / S by default), desktop notifications, device manager, log retention.

## Install

**Ubuntu 22.04 / 24.04**

```bash
sudo apt install ./ENRecorder_1.0.0_amd64.deb   # pulls in ffmpeg and the PipeWire GStreamer plugin
```

or the AppImage (needs `ffmpeg` installed: `sudo apt install ffmpeg`):

```bash
chmod +x ENRecorder_1.0.0_amd64.AppImage && ./ENRecorder_1.0.0_amd64.AppImage
```

OCR in the screenshot editor needs `sudo apt install tesseract-ocr` (add `tesseract-ocr-ben`
for Bengali). Transcription needs whisper.cpp (`whisper-cli`) and a speech model
(`ggml-base.bin` or similar); see [TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md#transcription).

**Windows 10 / 11**: run `ENRecorder_1.0.0_x64-setup.exe`. It installs for the current user (no
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
