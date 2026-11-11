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
