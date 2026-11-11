# ENRecorder — Security and privacy

ENRecorder is local-first. This page lists what it can reach and how each boundary is kept.
Reviewed in phase 7 (2026-11-11).

## Network

| What | When | Where |
|---|---|---|
| Update check | shortly after start, at most once a day, only while **Settings → Check for updates** is on (or when you press **Check now**) | `api.github.com` (latest release of ENRecorder) |
| Update download | only when you press **Install** | the release asset on GitHub; its SHA-256 (published by GitHub) is checked before anything runs |
| Preview server | while the app runs | listens on `127.0.0.1` only (see below) |

There is no account, no telemetry, no crash upload and no analytics. Recordings, screenshots
and the database never leave the machine.

## Processes

* FFmpeg, FFprobe, GStreamer (`gst-launch-1.0`, Wayland) and Tesseract are started with
  **argument arrays**, never through a shell, so file names and device names cannot inject
  commands. All of them go through one helper (`media/process.rs`), which also hides console
  windows on Windows.
* The updater runs `pkexec apt-get install <verified .deb>` (Ubuntu shows its own password
  prompt), replaces the AppImage atomically, or runs the verified NSIS installer with `/S`.
  It refuses while a recording is running.

## The window (web view)

* Content Security Policy: scripts only from the app itself; no remote origins. Media and images
  may come from `asset:` (thumbnails, screenshots) and `http://127.0.0.1:*` (preview server).
* Tauri capabilities: window controls and the folder picker only. Files, processes and the
  network are reached only through ENRecorder's own commands, which check their arguments:
  * `open_path` opens folders and media files only (never programs, scripts or `.desktop` files);
  * `open_url` opens `https://` and `mailto:` links only;
  * renames refuse path separators and reserved names; library actions take item ids, not paths.
* The asset protocol scope is empty in the configuration and is widened at run time to
  ENRecorder's own folders only: the recordings folder, the screenshots folder and the thumbnail
  cache.

## Preview server

WebKitGTK cannot play videos from Tauri's `asset:` scheme, so the library preview is served over
HTTP. It binds `127.0.0.1` on a random port, every URL contains a random 128-bit token made at
start-up, and it serves only items that are in the library database (by id), read-only. Other
programs on the machine cannot list or fetch anything without the token, and nothing listens on
the network.

## Files

* Delete moves files to the system **trash**; nothing is erased outright.
* Interrupted recordings are kept as parts in a hidden folder next to the recordings until you
  recover or discard them (Home shows a banner).
* Screenshot originals are kept next to the edited file (`.enrecorder-originals/`) so edits can
  be undone; they are removed with the screenshot.
* Settings and the library live in an SQLite database in the app data folder.

## Installers

* Ubuntu: `.deb` (depends on the distribution's `ffmpeg`) and AppImage (uses the system FFmpeg).
* Windows: NSIS installer for the current user (no administrator rights). It ships FFmpeg and
  FFprobe from BtbN/FFmpeg-Builds; the build script checks them against the release's
  `checksums.sha256` before they are bundled.

## Reporting a problem

Email info@mestiaque.com with the steps and, if possible, the text from
**Settings → About → Copy diagnostics** (it contains no recordings and no file contents).
