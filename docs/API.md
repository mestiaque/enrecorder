# ENRecorder — Commands and events

All commands are invoked with `invoke(name, args)` (see `src/lib/api.ts`). Errors reject with
`{ kind: string, message: string }`; `kind` is one of `ffmpegMissing`, `ffmpeg`, `diskFull`,
`invalidPath`, `invalidSetting`, `permissionDenied`, `unsupported`, `notFound`, `db`, `io`, `other`.
TypeScript types live in `src/lib/types.ts` and mirror the Rust structs (camelCase).

## Settings

| Command | Arguments | Returns |
|---|---|---|
| `get_settings` | — | `SettingsPayload { settings, defaultRecordings, defaultScreenshots }` |
| `set_setting` | `key: keyof Settings, value` | `Settings` (validated; folder keys must be writable full paths; `ffmpegPath` must be a file) |
| `reset_settings` | — | `SettingsPayload` |

## Media tools

| Command | Arguments | Returns |
|---|---|---|
| `media_tools` | — | `MediaTools { ffmpeg?, ffprobe?, capabilities?, checking }` |
| `verify_encoders` | — | starts a new test of every encoder in the background |

## Dashboard and library

| Command | Arguments | Returns |
|---|---|---|
| `dashboard` | — | `Dashboard { stats, recentRecordings, recentScreenshots, recordings: FolderUsage, screenshots: FolderUsage, disk? }` |
| `list_media` | `query: { filter, search, sort, limit? }` | `MediaItem[]` — filter `all|video|audio|screenshot|favorites`, sort `newest|oldest|name|size|duration`; unknown values fall back to defaults; the search is a bound parameter with `%`/`_` escaped |
| `list_export_jobs` | — | `ExportJob[]` (newest first, max 200) |

## System

| Command | Arguments | Returns |
|---|---|---|
| `system_info` | — | `SystemInfo { os, osName, arch, displayServer, desktop, pipewire?, pulse?, renderNodes, monitors[] }` (re-detected each call) |
| `app_paths` | — | `{ data, logs, recordings, screenshots }` |
| `read_logs` | `maxLines?` | last lines of the log |
| `diagnostics` | — | text for bug reports |
| `about_info` | — | `{ build, tauriVersion, webviewVersion, platform, arch }` |
| `open_path` / `reveal_path` | `path` | opens a file / shows it in the file manager |
| `open_folder` | `path` (absolute) | creates the folder if needed and opens it |
| `open_url` | `url` (`https://` or `mailto:` only) | opens in the default app |
| `open_log_folder`, `hide_to_tray`, `quit_app`, `frontend_log` | — | — |

## Events

| Event | Payload | When |
|---|---|---|
| `tools-changed` | — | FFmpeg search or encoder test started or finished (the UI refetches `media_tools`) |

Planned events (phases 2–5): `recording-started`, `recording-paused`, `recording-resumed`,
`recording-stopped`, `recording-progress`, `audio-level`, `device-changed`, `capture-error`,
`export-progress`, `export-completed`, `export-failed`, `storage-warning` — throttled to at most
10 per second for the UI.
