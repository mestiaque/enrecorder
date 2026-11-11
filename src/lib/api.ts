/** Typed wrappers around the Rust commands: one function per command. */
import { invoke } from "@tauri-apps/api/core";
import type { AboutInfo, AppPaths, Dashboard, ExportJob, LibraryQuery, MediaItem, MediaTools, Settings, SettingsPayload, SystemInfo } from "./types";

export const api = {
  getSettings: () => invoke<SettingsPayload>("get_settings"),
  setSetting: <K extends keyof Settings>(key: K, value: Settings[K]) => invoke<Settings>("set_setting", { key, value }),
  resetSettings: () => invoke<SettingsPayload>("reset_settings"),

  mediaTools: () => invoke<MediaTools>("media_tools"),
  verifyEncoders: () => invoke<void>("verify_encoders"),

  dashboard: () => invoke<Dashboard>("dashboard"),
  listMedia: (query: LibraryQuery) => invoke<MediaItem[]>("list_media", { query }),
  listExportJobs: () => invoke<ExportJob[]>("list_export_jobs"),

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
export const EVENTS = { toolsChanged: "tools-changed" } as const;

/** Commands reject with `{ kind, message }` (AppError) or a plain string. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}
