/** Mirrors the Rust types sent by the commands (see docs/API.md). */

export interface Settings {
  startWithOs: boolean;
  startMinimized: boolean;
  closeToTray: boolean;
  language: "en";
  recordingsDir: string;
  screenshotsDir: string;
  ffmpegPath: string;
  theme: "dark" | "light" | "glass" | "system";
  accent: string;
  animations: boolean;
  sidebarCollapsed: boolean;
}

export interface SettingsPayload {
  settings: Settings;
  defaultRecordings: string;
  defaultScreenshots: string;
}

export type DisplayServer = "x11" | "wayland" | "windows" | "unknown";

export interface MonitorInfo {
  name: string;
  width: number;
  height: number;
  x: number;
  y: number;
  scale: number;
  primary: boolean;
}

export interface SystemInfo {
  os: string;
  osName: string;
  arch: string;
  displayServer: DisplayServer;
  desktop: string;
  pipewire: boolean | null;
  pulse: boolean | null;
  renderNodes: string[];
  monitors: MonitorInfo[];
}

export interface ToolInfo {
  path: string;
  version: string;
}

export type Codec = "h264" | "hevc" | "av1" | "vp9";
export type Vendor = "software" | "nvidia" | "amd" | "intel" | "vaapi" | "mediaFoundation";

export interface EncoderStatus {
  name: string;
  codec: Codec;
  vendor: Vendor;
  listed: boolean;
  verified: boolean;
  detail: string;
}

export interface MediaTools {
  ffmpeg: ToolInfo | null;
  ffprobe: ToolInfo | null;
  capabilities: { ffmpeg: ToolInfo; encoders: EncoderStatus[]; checkedAt: number } | null;
  checking: boolean;
}

export interface MediaItem {
  id: string;
  kind: "video" | "audio" | "screenshot";
  title: string;
  path: string;
  sizeBytes: number;
  durationMs: number | null;
  width: number | null;
  height: number | null;
  status: string;
  favorite: boolean;
  createdAt: number;
}

export interface LibraryStats {
  videos: number;
  audio: number;
  screenshots: number;
  totalDurationMs: number;
  totalBytes: number;
  last7Days: number;
  interrupted: number;
}

export interface FolderUsage {
  path: string;
  exists: boolean;
  bytes: number;
  files: number;
  truncated: boolean;
}

export interface DiskSpace {
  path: string;
  available: number;
  total: number;
}

export interface Dashboard {
  stats: LibraryStats;
  recentRecordings: MediaItem[];
  recentScreenshots: MediaItem[];
  recordings: FolderUsage;
  screenshots: FolderUsage;
  disk: DiskSpace | null;
}

export interface LibraryQuery {
  filter: "all" | "video" | "audio" | "screenshot" | "favorites";
  search: string;
  sort: "newest" | "oldest" | "name" | "size" | "duration";
  limit?: number;
}

export interface ExportJob {
  id: string;
  sourcePath: string;
  outputPath: string;
  status: "queued" | "initializing" | "running" | "finalizing" | "completed" | "failed" | "cancelled";
  progress: number;
  error: string | null;
  createdAt: number;
  finishedAt: number | null;
}

export interface AppPaths {
  data: string;
  logs: string;
  recordings: string;
  screenshots: string;
}

export interface AboutInfo {
  build: "debug" | "release";
  tauriVersion: string;
  webviewVersion: string;
  platform: string;
  arch: string;
}
