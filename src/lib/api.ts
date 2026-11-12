/** Typed wrappers around the Rust commands: one function per command. */
import { invoke } from "@tauri-apps/api/core";
import type { AboutInfo, AudioDevice, Camera, Interrupted, AudioRecordingOptions, ExportPreset, MediaDetail, Screenshot, AppPaths, CaptureSource, Dashboard, ExportJob, LibraryQuery, MediaItem, MediaTools, RecordingOptions, RecordingSnapshot, Settings, SettingsPayload, SourcesPayload, SystemInfo, Tag, ShortcutRow, DeviceRow, ScheduleInfo } from "./types";

export const api = {
  getSettings: () => invoke<SettingsPayload>("get_settings"),
  setSetting: <K extends keyof Settings>(key: K, value: Settings[K]) => invoke<Settings>("set_setting", { key, value }),
  resetSettings: () => invoke<SettingsPayload>("reset_settings"),

  mediaTools: () => invoke<MediaTools>("media_tools"),
  verifyEncoders: () => invoke<void>("verify_encoders"),

  dashboard: () => invoke<Dashboard>("dashboard"),
  listMedia: (query: LibraryQuery) => invoke<MediaItem[]>("list_media", { query }),
  listExportJobs: () => invoke<ExportJob[]>("list_export_jobs"),

  listCaptureSources: () => invoke<SourcesPayload>("list_capture_sources"),
  recordingStatus: () => invoke<RecordingSnapshot | null>("recording_status"),
  startRecording: (options: RecordingOptions) => invoke<RecordingSnapshot>("start_video_recording", { options }),
  listAudioDevices: () => invoke<AudioDevice[]>("list_audio_devices"),
  listCameras: () => invoke<Camera[]>("list_cameras"),
  checkUpdate: () => invoke<UpdateInfo | null>("check_update"),
  installUpdate: (info: UpdateInfo) => invoke<"restart" | "exiting">("install_update", { info }),
  restartApp: () => invoke<void>("restart_app"),
  listInterrupted: () => invoke<Interrupted[]>("list_interrupted"),
  recoverRecording: (sessionId: string) => invoke<string>("recover_recording", { sessionId }),
  discardRecording: (sessionId: string) => invoke<void>("discard_recording", { sessionId }),
  ocrStatus: () => invoke<{ available: boolean; languages: string[] }>("ocr_status"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  ocrScreenshot: (png: string, languages: string) => invoke<string>("ocr_screenshot", { png, languages }),
  takeScreenshot: (source: CaptureSource | null, delay: number, hideWindow: boolean, cursor: boolean) => invoke<Screenshot>("take_screenshot", { source, delay, hideWindow, cursor }),
  screenshotImage: (id: string) => invoke<string>("screenshot_image", { id }),
  saveScreenshot: (id: string, png: string, format: string, quality: number, annotations: string | null) => invoke<Screenshot>("save_screenshot", { id, png, format, quality, annotations }),
  copyScreenshot: (png: string) => invoke<void>("copy_screenshot", { png }),
  getScreenshot: (id: string) => invoke<Screenshot>("get_screenshot", { id }),
  getMedia: (id: string) => invoke<MediaDetail>("get_media", { id }),
  mediaUrl: (id: string) => invoke<string>("media_url", { id }),
  renameMedia: (id: string, title: string) => invoke<MediaItem>("rename_media", { id, title }),
  deleteMedia: (id: string) => invoke<void>("delete_media", { id }),
  setFavorite: (id: string, on: boolean) => invoke<void>("set_favorite", { id, on }),
  mediaThumbnail: (id: string) => invoke<string>("media_thumbnail", { id }),
  startExport: (id: string, preset: ExportPreset) => invoke<string>("start_export", { id, preset }),
  cancelExport: (id: string) => invoke<void>("cancel_export", { id }),
  startAudioRecording: (options: AudioRecordingOptions) => invoke<RecordingSnapshot>("start_audio_recording", { options }),
  pauseRecording: () => invoke<void>("pause_recording"),
  resumeRecording: () => invoke<void>("resume_recording"),
  stopRecording: () => invoke<void>("stop_recording"),
  openRegionSelector: () => invoke<void>("open_region_selector"),
  regionSelected: (rect: CaptureSource | null) => invoke<void>("region_selected", { rect }),

  systemInfo: () => invoke<SystemInfo>("system_info"),
  appPaths: () => invoke<AppPaths>("app_paths"),
  readLogs: (maxLines = 500) => invoke<string>("read_logs", { maxLines }),
  diagnostics: () => invoke<string>("diagnostics"),
  aboutInfo: () => invoke<AboutInfo>("about_info"),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openLogFolder: () => invoke<void>("open_log_folder"),
  quitApp: () => invoke<void>("quit_app"),
  hideToTray: () => invoke<void>("hide_to_tray"),

  scheduleRecording: (options: RecordingOptions, atMs: number, label: string) => invoke<ScheduleInfo>("schedule_recording", { options, atMs, label }),
  scheduleStatus: () => invoke<ScheduleInfo | null>("schedule_status"),
  cancelSchedule: () => invoke<void>("cancel_schedule"),
  deviceManager: () => invoke<DeviceRow[]>("device_manager"),
  deleteMediaMany: (ids: string[]) => invoke<number>("delete_media_many", { ids }),
  listTags: () => invoke<Tag[]>("list_tags"),
  createTag: (name: string, color: string | null = null) => invoke<Tag>("create_tag", { name, color }),
  deleteTag: (id: string) => invoke<void>("delete_tag", { id }),
  setMediaTags: (ids: string[], tagIds: string[]) => invoke<void>("set_media_tags", { ids, tagIds }),
  mediaTagIds: (id: string) => invoke<string[]>("media_tag_ids", { id }),
  checkLibraryFiles: () => invoke<[number, number]>("check_library_files"),
  reprocessMedia: (id: string) => invoke<MediaItem>("reprocess_media", { id }),
  mediaTrack: (id: string) => invoke<{ clicks: boolean; keys: boolean } | null>("media_track", { id }),
  extractFrame: (id: string, atMs: number) => invoke<string>("extract_frame", { id, atMs }),
  estimateExport: (id: string, preset: ExportPreset) => invoke<number | null>("estimate_export", { id, preset }),
  retryExport: (id: string) => invoke<string>("retry_export", { id }),
  detectSilence: (id: string) => invoke<[number, number | null][]>("detect_silence", { id }),
  getShortcuts: () => invoke<ShortcutRow[]>("get_shortcuts"),
  setShortcut: (action: string, accelerator: string | null) => invoke<ShortcutRow[]>("set_shortcut", { action, accelerator }),
  resetShortcuts: () => invoke<ShortcutRow[]>("reset_shortcuts"),
  transcriptionStatus: () => invoke<TranscriptionStatus>("transcription_status"),
  transcribeMedia: (id: string) => invoke<string>("transcribe_media", { id }),
  cancelTranscription: () => invoke<void>("cancel_transcription"),
  startQuickCapture: () => invoke<void>("start_quick_capture"),
  quickCaptureCancel: () => invoke<void>("quick_capture_cancel"),
  quickCaptureFinish: (action: "copy" | "save" | "edit", png: string, annotations: string, copy: boolean) => invoke<string | null>("quick_capture_finish", { action, png, annotations, copy }),
};

/** whisper.cpp (optional): program and models found, and the running job. */
export interface TranscriptionStatus {
  program: string | null;
  models: string[];
  modelFolder: string;
  running: string | null;
}

/** Event names emitted by the backend. */
export interface UpdateInfo {
  version: string;
  notes: string;
  page: string;
  assetName: string;
  url: string;
  size: number;
  sha256: string;
  installable: boolean;
}

export const EVENTS = {
  toolsChanged: "tools-changed",
  recordingState: "recording-state",
  recordingProgress: "recording-progress",
  regionSelected: "region-selected",
  startRequest: "recording-start-request",
  confirmQuit: "confirm-quit",
  exportProgress: "export-progress",
  scheduleChanged: "schedule-changed",
  screenshotRequest: "screenshot-request",
  transcribeProgress: "transcribe-progress",
  openScreenshot: "open-screenshot",
} as const;

/** Commands reject with `{ kind, message }` (AppError) or a plain string. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}
