# ENRecorder — Architecture

ENRecorder ("Capture Studio" in the original brief) records the screen, audio and screenshots on
Windows 10/11 and Ubuntu 22.04/24.04 (X11 and Wayland), then lets you trim, convert and export
the results. It is a member of the EN* family (ENViewer, ENPlayer, ENSave, ENStream, ENiShare)
and shares their look, theme tokens and settings layout.

This document describes what exists and the boundaries later phases build on. Anything not
built yet is marked **(planned, phase N)**.

## 1. Process layout

```
┌──────────────────────── ENRecorder (one process) ─────────────────────────┐
│  WebView (React UI)            │  Rust (Tauri 2)                          │
│  pages · stores · components   │  commands  ─┐                            │
│        ▲  invoke / events      │  AppState   ├─ enrec-core (no Tauri)     │
│        └───────────────────────┤  tray       │   database · settings      │
│                                │  shortcuts  │   media (FFmpeg/FFprobe)   │
│                                │             │   system · storage         │
│                                │             │   capture traits           │
└────────────────────────────────┴─────────────┴────────────────────────────┘
          │ child processes (argument arrays, never a shell)
          ▼
      ffmpeg / ffprobe
```

* **`crates/enrec-core`** — everything that does not need a window: SQLite store and
  migrations, typed settings, the FFmpeg service, system detection, storage accounting and the
  capture interfaces. Unit-tested without Tauri. Platform code lives in `cfg`-gated modules
  (`…/linux.rs`, `…/windows.rs`), never mixed in one file.
* **`src-tauri`** — the desktop shell: Tauri commands (thin wrappers around core services),
  app state, tray, window handling, logging.
* **`src`** — the React UI.

## 2. Rust services

| Service | Where | Responsibility | Status |
|---|---|---|---|
| `Db` | core `database` | SQLite (WAL, foreign keys), versioned migrations, startup recovery | done |
| `Settings` | core `settings` | typed settings, per-key validation, defaults | done |
| `FfmpegService` | core `media::ffmpeg` | locate + verify FFmpeg/FFprobe, version, encoder list, **verified** encoders | done |
| `SystemInfo` | core `system` | OS, display server (X11/Wayland), desktop, PipeWire, GPU render nodes | done |
| `StorageService` | core `storage` | folder usage, free disk space | done |
| `CaptureSessionManager` | core `capture` | state machine + platform backends | planned, phase 2 |
| `AudioSessionManager` | core `audio` | CPAL/WASAPI/PipeWire capture, mixer, tracks | planned, phase 3 |
| `ScreenshotService` | core `screenshot` | capture + annotation export | planned, phase 4 |
| `MediaService` / `ExportJobManager` | core `media` | FFprobe metadata, thumbnails, export queue | planned, phase 5 |
| `RecoveryService` | core `database` + `media` | interrupted recordings | startup marking done; finalisation phase 7 |

`AppState` (src-tauri) owns one instance of each and is shared through Tauri's managed state.
Long work (FFmpeg probes, folder walks) runs on `tauri::async_runtime::spawn_blocking`, never on
the main thread.

## 3. Capture abstraction (interface defined, backends phase 2)

```rust
trait CaptureBackend: Send {
    fn kind(&self) -> BackendKind;                 // WindowsGraphicsCapture | X11 | WaylandPortal
    fn list_sources(&self) -> Result<Vec<CaptureSource>>;
    fn start(&mut self, req: CaptureRequest, sink: FrameSink) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<CaptureSummary>;
}
```

* **Windows:** Windows.Graphics.Capture (screen/window), WASAPI + loopback (audio), Media
  Foundation (webcam), via the `windows` crate.
* **Linux X11:** a dedicated X11 backend (XShm grab of a monitor/region).
* **Linux Wayland:** xdg-desktop-portal ScreenCast → PipeWire stream; the user picks the source in
  the portal dialog. X11 support is never assumed to mean Wayland support.

The backend is chosen from the detected display server (`system::display`). Frames go through a
**bounded** queue to the encoder; when the encoder falls behind, the oldest *unencoded* frame is
dropped and counted (shown in the HUD), so memory stays bounded.

### Recording state machine (phase 2)

```
Idle → Preparing → Countdown → Recording ⇄ Paused → Finalizing → Completed
                     │             │          │          └→ Failed
                     └─────────────┴──────────┴→ Cancelled / Failed
```

## 4. Data model (SQLite)

UUID (text) primary keys, foreign keys on, one migration per schema version recorded in
`schema_migrations`. Media files live on disk; the database only stores paths and metadata.

| Table | Purpose |
|---|---|
| `recordings` | one row per video/audio file: path, kind, duration, size, status |
| `screenshots` | one row per image: path, size, dimensions, editable annotation JSON |
| `media_metadata` | FFprobe facts for a recording (codec, container, streams, bitrate) |
| `capture_sessions` | a recording attempt: state, source, settings snapshot, timestamps |
| `export_jobs` | export queue: source, preset, status, progress, error, output |
| `devices` | audio/video devices seen, defaults and last connection state |
| `settings` | key → JSON value (per-key validation, no migration per new setting) |
| `shortcuts` | global shortcut per action |
| `tags`, `media_tags`, `favorites` | library organisation (tag/favourite a recording **or** a screenshot) |

`media_tags` and `favorites` point at a recording *or* a screenshot through two nullable foreign
keys with a `CHECK` that exactly one is set, so referential integrity holds for both.

**Startup recovery:** sessions left in `preparing/recording/paused/finalizing` by a crash are
marked `interrupted`; their files are never deleted (phase 7 adds container finalisation).

## 5. FFmpeg integration

* Found in this order: path set in Settings → bundled copy next to the program / in the data
  folder → `PATH`. Each candidate must answer `-version`.
* Encoders listed by `ffmpeg -encoders` are only **candidates**. An encoder is shown as available
  only after a real test encode (a short synthetic clip to the null muxer, with the hardware
  setup it needs, e.g. VAAPI render node, QSV device). Results are cached per FFmpeg path+version.
* All invocations use argument arrays (`std::process::Command`), never a shell, and no part of a
  command line comes from the UI except validated paths.
* Licensing: on Ubuntu the distribution's `ffmpeg` package is a dependency; on Windows a separate
  FFmpeg build is shipped next to the program (planned, phase 7) with its licence.

## 6. Frontend

* React 18 + TypeScript (strict) + Vite + Tailwind 3, Radix primitives, Lucide icons — the same
  component set as ENSave/ENViewer (`components/ui`), so dialogs, menus, settings and themes look
  identical across the family. (shadcn/ui is the same idea — Radix + Tailwind components kept in
  the repo — so the family components are used instead of a second copy.)
* **State:** Zustand for UI/session state; TanStack Query for data that comes from the backend
  (dashboard, library, system readiness), with explicit invalidation after mutations.
* Pages: Home, Video Recorder, Audio Recorder, Screenshot Studio, Media Editor, Recordings,
  Export Center; Settings is a dialog (family layout).
* React Hook Form + Zod and Framer Motion from the brief are added with the first feature that
  needs them (the recorder setup form, phase 2), not before, to avoid unused dependencies.

## 7. Commands and events

See `API.md` for request/response types. Commands are grouped by area (`settings`, `system`,
`dashboard`, `library`, later `video`, `audio`, `screenshot`, `media`, `export`). Every command
returns `Result<T, AppError>`; errors reach the UI as `{ kind, message }`.

Continuous updates use Tauri events (`recording-progress`, `audio-level`, `export-progress`, …,
phase 2+), throttled to ≤10 per second for the UI while capture keeps full precision internally.

## 8. Security

* Tauri capabilities list only what the window uses (window controls, dialogs). No shell plugin;
  child processes are started by Rust with fixed program paths and argument arrays.
* Paths from the UI are validated (absolute, existing directory, writable) before use.
* Nothing is uploaded; there is no telemetry.
