/** Mirrors the Rust types sent by the commands (see docs/API.md). */

export interface Settings {
  startWithOs: boolean;
  startMinimized: boolean;
  closeToTray: boolean;
  language: "en";
  recordingsDir: string;
  screenshotsDir: string;
  ffmpegPath: string;
  recFps: 24 | 30 | 60 | 120;
  recResolution: "native" | "720" | "1080" | "1440" | "2160";
  recFormat: "mp4" | "mkv" | "webm";
  recCodec: Codec;
  recEncoder: string;
  recQuality: "high" | "balanced" | "small";
  recCursor: boolean;
  recCountdown: 0 | 3 | 5 | 10;
  recMaxMinutes: number;
  recHideWindow: boolean;
  recShowHud: boolean;
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

export type CaptureSource =
  | { type: "desktop"; width: number; height: number }
  | { type: "monitor"; id: string; name: string; width: number; height: number; x: number; y: number; primary: boolean }
  | { type: "window"; id: string; title: string; app: string; width: number; height: number }
  | { type: "region"; x: number; y: number; width: number; height: number };

export interface SourcesPayload {
  backend: "x11" | "waylandPortal" | "windowsGraphicsCapture" | null;
  unsupported: string | null;
  sources: CaptureSource[];
}

export type RecState = "idle" | "preparing" | "countdown" | "recording" | "paused" | "finalizing" | "completed" | "failed" | "cancelled";

export interface RecordingSnapshot {
  sessionId: string;
  state: RecState;
  countdownLeft: number;
  elapsedMs: number;
  segments: number;
  frames: number;
  dropped: number;
  fps: number;
  sizeBytes: number;
  encoder: string;
  width: number;
  height: number;
  source: string;
  outputPath: string | null;
  recordingId: string | null;
  error: string | null;
  notice: string | null;
}

export interface RecordingOptions {
  source: CaptureSource;
  fps: number;
  resolution: string;
  format: string;
  codec: Codec;
  encoder: string;
  quality: string;
  showCursor: boolean;
  countdown: number;
  maxMinutes: number;
}
