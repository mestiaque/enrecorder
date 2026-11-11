/** Typed wrappers around the Rust commands: one function per command. */
import { invoke } from "@tauri-apps/api/core";
import type { AboutInfo, AudioDevice, AudioRecordingOptions, AppPaths, CaptureSource, Dashboard, ExportJob, LibraryQuery, MediaItem, MediaTools, RecordingOptions, RecordingSnapshot, Settings, SettingsPayload, SourcesPayload, SystemInfo } from "./types";

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
};

/** Event names emitted by the backend. */
export const EVENTS = {
  toolsChanged: "tools-changed",
  recordingState: "recording-state",
  recordingProgress: "recording-progress",
  regionSelected: "region-selected",
  startRequest: "recording-start-request",
  confirmQuit: "confirm-quit",
} as const;

/** Commands reject with `{ kind, message }` (AppError) or a plain string. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}
