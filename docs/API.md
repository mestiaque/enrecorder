# ENRecorder — Commands and events

All commands are invoked with `invoke(name, args)` (see `src/lib/api.ts`). Errors reject with
`{ kind: string, message: string }`; `kind` is one of `ffmpegMissing`, `ffmpeg`, `diskFull`,
`invalidPath`, `invalidSetting`, `permissionDenied`, `unsupported`, `notFound`, `db`, `io`, `other`,
`cancelled`.
TypeScript types live in `src/lib/types.ts` and mirror the Rust structs (camelCase).

## Settings

| Command | Arguments | Returns |
|---|---|---|
| `get_settings` | — | `SettingsPayload { settings, defaultRecordings, defaultScreenshots }` |
| `set_setting` | `key: keyof Settings, value` | `Settings` (validated; folder keys must be writable full paths; `ffmpegPath` must be a file) |
| `reset_settings` | — | `SettingsPayload` |
| `get_shortcuts` | — | `ShortcutRow[]` = `{ action, label, accelerator \| null, default }` |
| `set_shortcut` | `action`, `accelerator \| null` (null = off) | `ShortcutRow[]`; refused when another action uses it or the system will not register it (the old one is restored) |
| `reset_shortcuts` | — | `ShortcutRow[]` (defaults) |

## Media tools

| Command | Arguments | Returns |
|---|---|---|
| `media_tools` | — | `MediaTools { ffmpeg?, ffprobe?, capabilities?, checking }` |
| `verify_encoders` | — | starts a new test of every encoder in the background |

## Dashboard and library

| Command | Arguments | Returns |
|---|---|---|
| `dashboard` | — | `Dashboard { stats, recentRecordings, recentScreenshots, recordings: FolderUsage, screenshots: FolderUsage, disk? }` |
| `list_media` | `query: { filter, search, sort, limit?, date?, length?, size?, resolution?, tag? }` | `MediaItem[]` — filter `all|video|audio|screenshot|favorites`, sort `newest|oldest|name|size|duration`; unknown values fall back to defaults; the search is a bound parameter with `%`/`_` escaped |
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
| `schedule_recording` | `options: RecordingOptions`, `atMs` (epoch ms, in the future), `label` | `ScheduleInfo { atMs, label }`; one at a time |
| `schedule_status` / `cancel_schedule` | — | `ScheduleInfo \| null` / — |
| `device_manager` | — | `DeviceRow[]` = `{ kind, backendId, name, isDefault, connected, details, lastSeen }` (devices table, refreshed on each call) |
| `list_cameras` | — | `Camera[]` = `{ id, name }` |
| `list_audio_devices` | — | `AudioDevice[]` = `{ id, name, kind: "microphone"\|"system", default, channels?, sampleRate? }` |
| `start_audio_recording` | `options: AudioRecordingOptions` | `RecordingSnapshot` (audio only; pause/resume/stop as above) |

`AudioOptions` = `{ microphone?: id, system?: id, separateTracks, micVolume, systemVolume (percent, 0–200),
micDenoise, micNormalize, micGate, echoCancel, sampleRate (44100|48000), channels (1|2) }`; `WebcamOptions` = `{ device?: id, position, size (10–40 %), shape: "circle"|"rectangle"|"rounded", border? ("#rrggbb"), shadow }`
(in `RecordingOptions.webcam`);
`AudioRecordingOptions` = `{ audio: AudioOptions, format ("mp3"|"m4a"|"opus"|"ogg"|"flac"|"wav"), quality, countdown, maxMinutes }`.

`CaptureSource` = `{type:"desktop",width,height}` · `{type:"monitor",id,name,width,height,x,y,primary}` ·
`{type:"window",id,title,app,width,height}` · `{type:"region",x,y,width,height}` (physical pixels) ·
`{type:"camera",id,name}` · `{type:"portal"}` (Wayland).

`RecordingOptions` = `{ source, fps (24|30|60|120), resolution ("native"|"720"|"1080"|"1440"|"2160"),
format ("mp4"|"mkv"|"webm"), codec ("h264"|"hevc"|"av1"|"vp9"), encoder ("auto"|name), quality
("high"|"balanced"|"small"), showCursor, countdown (s), maxMinutes (0 = none), audio?: AudioOptions, webcam?: WebcamOptions,
extras: { splitMinutes (0 = off), softwareOnly, trackClicks, trackKeys } }`.

`RecordingSnapshot` = `{ sessionId, state, countdownLeft, elapsedMs, segments, frames, dropped, fps,
sizeBytes, encoder, width, height, source, outputPath?, recordingId?, error?, notice?, audioLevel? (dBFS),
audioRms?, clipped, warning?, audioTracks, audioOnly }`. `notice` carries e.g. "Saved as 3 files" after splitting.

## Library and exports

| Command | Arguments | Returns |
|---|---|---|
| `get_media` | `id` | `{ item: MediaItem, metadata?: Probe, exists }` |
| `media_url` | `id` | preview URL (`http://127.0.0.1:<port>/m/<token>/<id>`) |
| `media_thumbnail` | `id` | path of the cached JPEG |
| `rename_media` | `id`, `title` | `MediaItem` |
| `delete_media` | `id` | — (file to the trash) |
| `delete_media_many` | `ids` | number deleted |
| `list_tags` / `create_tag` / `delete_tag` | — / `name`, `color?` / `id` | `Tag[]` (with counts) / `Tag` / — |
| `set_media_tags` / `media_tag_ids` | `ids`, `tagIds` / `id` | — / tag ids |
| `check_library_files` | — | `[newlyMissing, back]` (items marked or restored) |
| `reprocess_media` | `id` | `MediaItem` (FFprobe again, new thumbnail) |
| `media_track` | `id` | `{ clicks, keys } \| null` — is there a pointer track for this recording |
| `extract_frame` | `id`, `atMs` | id of the new screenshot |
| `detect_silence` | `id` | `[startMs, endMs \| null][]` (below −45 dB for 0.7 s or longer) |
| `transcription_status` | — | `{ program?, models[], modelFolder, running? }` |
| `transcribe_media` | `id` | path of the new .srt (one at a time; `cancelled` error when stopped) |
| `cancel_transcription` | — | — |
| `set_favorite` | `id`, `on` | — |
| `start_export` | `id`, `preset: ExportPreset` | job id |
| `cancel_export` | `id` (job) | — |
| `retry_export` | `id` (failed/cancelled job) | new job id |
| `estimate_export` | `id`, `preset` | estimated bytes or null |

`ExportPreset` = `{ kind: "convert"|"compress"|"audio"|"gif"|"trim", format, quality, startMs?, endMs?,
maxHeight?, removeAudio?, volume?, denoise?, normalize?, crop?: {x,y,width,height}, rotate (0|90|180|270),
speed (0.25–4), fadeInMs, fadeOutMs, fps?, videoCodec?, videoKbps?, audioKbps?, clickHighlights, keystrokes }`
(`overlayPath` is set by the app). The same export of the same item while one is queued is refused. Event `export-progress`: `{ id, status, progress, error?, outputPath }`.

## Recovery and updates

| Command | Arguments | Returns |
|---|---|---|
| `list_interrupted` | — | `Interrupted[]` = `{ sessionId, kind, startedAt, folder, parts, sizeBytes }` |
| `recover_recording` | `sessionId` | id of the new library item "<title> (recovered).mkv" |
| `discard_recording` | `sessionId` | — (parts deleted) |
| `check_update` | — | `UpdateInfo \| null` (null = up to date) = `{ version, notes, page, assetName, url, size, sha256, installable }` |
| `install_update` | `info: UpdateInfo` | message (download, SHA-256 check, install; refused while recording) |
| `restart_app` | — | — |

## Screenshots

| Command | Arguments | Returns |
|---|---|---|
| `take_screenshot` | `source: CaptureSource \| null` (null = Wayland portal), `delay` (s), `hideWindow`, `cursor` | `Screenshot` |
| `screenshot_image` | `id` | base64 PNG of the original |
| `save_screenshot` | `id`, `png` (base64/data URL), `format` ("png"\|"jpeg"\|"webp"), `quality`, `annotations?` (JSON) | `Screenshot` |
| `copy_screenshot` | `png` | — |
| `ocr_status` | — | `{ available, languages }` |
| `ocr_screenshot` | `png`, `languages` ("eng+ben") | recognised text |
| `copy_text` | `text` | — |
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
| `export-progress` | `{ id, status, progress, error?, outputPath }` | export job changes |
| `schedule-changed` | `ScheduleInfo \| null` | a recording was scheduled, started or cancelled |
| `screenshot-request` | — | the screenshot shortcut was pressed |
| `transcribe-progress` | `{ mediaId, percent }` | transcription progress |
