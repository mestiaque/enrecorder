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

## Video recording

| Command | Arguments | Returns |
|---|---|---|
| `list_capture_sources` | — | `{ backend: "x11" \| "waylandPortal" \| "windowsGraphicsCapture" \| null, unsupported?: string, sources: CaptureSource[] }` |
| `start_video_recording` | `options: RecordingOptions` | `RecordingSnapshot` (the session starts; recording begins after the countdown) |
| `pause_recording` / `resume_recording` | — | — |
| `stop_recording` | — | — (during the countdown this cancels) |
| `recording_status` | — | `RecordingSnapshot \| null` (current or last session) |
| `open_region_selector` | — | opens the area overlay on the main window's monitor |
| `region_selected` | `rect: CaptureSource(region) \| null` | called by the overlay; emits `region-selected` |

| `list_audio_devices` | — | `AudioDevice[]` = `{ id, name, kind: "microphone"\|"system", default }` |
| `start_audio_recording` | `options: AudioRecordingOptions` | `RecordingSnapshot` (audio only; pause/resume/stop as above) |

`AudioOptions` = `{ microphone?: id, system?: id, separateTracks, micVolume, systemVolume (percent, 0–200) }`;
`AudioRecordingOptions` = `{ audio: AudioOptions, format ("mp3"|"m4a"|"opus"|"flac"|"wav"), quality, countdown, maxMinutes }`.

`CaptureSource` = `{type:"desktop",width,height}` · `{type:"monitor",id,name,width,height,x,y,primary}` ·
`{type:"window",id,title,app,width,height}` · `{type:"region",x,y,width,height}` (physical pixels).

`RecordingOptions` = `{ source, fps (24|30|60|120), resolution ("native"|"720"|"1080"|"1440"|"2160"),
format ("mp4"|"mkv"|"webm"), codec ("h264"|"hevc"|"av1"|"vp9"), encoder ("auto"|name), quality
("high"|"balanced"|"small"), showCursor, countdown (s), maxMinutes (0 = none), audio?: AudioOptions }`.

`RecordingSnapshot` = `{ sessionId, state, countdownLeft, elapsedMs, segments, frames, dropped, fps,
sizeBytes, encoder, width, height, source, outputPath?, recordingId?, error?, notice?, audioLevel? (dBFS),
audioTracks, audioOnly }`.

## Library and exports

| Command | Arguments | Returns |
|---|---|---|
| `get_media` | `id` | `{ item: MediaItem, metadata?: Probe, exists }` |
| `media_url` | `id` | preview URL (`http://127.0.0.1:<port>/m/<token>/<id>`) |
| `media_thumbnail` | `id` | path of the cached JPEG |
| `rename_media` | `id`, `title` | `MediaItem` |
| `delete_media` | `id` | — (file to the trash) |
| `set_favorite` | `id`, `on` | — |
| `start_export` | `id`, `preset: ExportPreset` | job id |
| `cancel_export` | `id` (job) | — |

`ExportPreset` = `{ kind: "convert"|"compress"|"audio"|"gif"|"trim", format, quality, startMs?, endMs?,
maxHeight?, removeAudio?, volume? }`. Event `export-progress`: `{ id, status, progress, error?, outputPath }`.

## Screenshots

| Command | Arguments | Returns |
|---|---|---|
| `take_screenshot` | `source: CaptureSource \| null` (null = Wayland portal), `delay` (s), `hideWindow`, `cursor` | `Screenshot` |
| `screenshot_image` | `id` | base64 PNG of the original |
| `save_screenshot` | `id`, `png` (base64/data URL), `format` ("png"\|"jpeg"\|"webp"), `quality`, `annotations?` (JSON) | `Screenshot` |
| `copy_screenshot` | `png` | — |
| `get_screenshot` | `id` | `Screenshot` = `{ id, title, path, format, width, height, sizeBytes, annotations?, createdAt }` |

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
| `recording-state` | `RecordingSnapshot` | every state change, and once per countdown second |
| `recording-progress` | `RecordingSnapshot` | timer/size/frames, at most 4 per second |
| `region-selected` | `CaptureSource \| null` | the area overlay finished (null = cancelled) |
| `recording-start-request` | — | Ctrl+Shift+R or tray asks the main window to start with its form |
| `confirm-quit` | — | quit was requested while recording; the UI asks before calling `quit_app` |

Planned events (phases 3–5): `audio-level`, `device-changed`,
`export-progress`, `export-completed`, `export-failed`, `storage-warning` — throttled to at most
10 per second for the UI.
