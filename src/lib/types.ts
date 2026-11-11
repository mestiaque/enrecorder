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
  recMic: string;
  recSystemAudio: string;
  recSeparateTracks: boolean;
  checkUpdates: boolean;
  lastUpdateCheck: number;
  recMicDenoise: boolean;
  recMicNormalize: boolean;
  recWebcam: string;
  recWebcamPosition: "bottom-right" | "bottom-left" | "top-right" | "top-left";
  recWebcamSize: number;
  recWebcamShape: "circle" | "rectangle";
  audioMic: string;
  audioSystem: string;
  audioFormat: AudioFormat;
  audioQuality: "high" | "balanced" | "small";
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
  | { type: "region"; x: number; y: number; width: number; height: number }
  | { type: "portal" };

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
  /** Peak level of the last 0.1 s in dBFS (-120…0) while audio is recorded. */
  audioLevel: number | null;
  audioTracks: number;
  audioOnly: boolean;
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
  audio?: AudioOptions;
  webcam?: WebcamOptions;
}

export type AudioFormat = "mp3" | "m4a" | "opus" | "flac" | "wav";

export interface AudioDevice {
  id: string;
  name: string;
  kind: "microphone" | "system";
  default: boolean;
}

export interface AudioOptions {
  microphone: string | null;
  system: string | null;
  separateTracks: boolean;
  micVolume: number;
  systemVolume: number;
  micDenoise?: boolean;
  micNormalize?: boolean;
}

export interface WebcamOptions {
  device: string | null;
  position: Settings["recWebcamPosition"];
  size: number;
  shape: Settings["recWebcamShape"];
}

export interface Camera {
  id: string;
  name: string;
}

export interface AudioRecordingOptions {
  audio: AudioOptions;
  format: AudioFormat;
  quality: string;
  countdown: number;
  maxMinutes: number;
}

export interface Screenshot {
  id: string;
  title: string;
  path: string;
  format: "png" | "jpeg" | "webp";
  width: number;
  height: number;
  sizeBytes: number;
  annotations: string | null;
  createdAt: number;
}

export interface Probe {
  container: string | null;
  videoCodec: string | null;
  audioCodec: string | null;
  width: number | null;
  height: number | null;
  fps: number | null;
  durationMs: number | null;
  bitrate: number | null;
  videoStreams: number;
  audioStreams: number;
}

export interface MediaDetail {
  item: MediaItem;
  metadata: Probe | null;
  exists: boolean;
}

export interface ExportPreset {
  kind: "convert" | "compress" | "audio" | "gif" | "trim";
  format: "mp4" | "webm" | "mkv" | "gif" | "mp3" | "m4a" | "wav" | "flac" | "opus";
  quality: "high" | "balanced" | "small";
  startMs?: number | null;
  endMs?: number | null;
  maxHeight?: number;
  removeAudio?: boolean;
  volume?: number;
  denoise?: boolean;
  normalize?: boolean;
}

export interface ExportUpdate {
  id: string;
  status: ExportJob["status"];
  progress: number;
  error: string | null;
  outputPath: string;
}

export interface Interrupted {
  sessionId: string;
  kind: "video" | "audio";
  startedAt: number;
  folder: string | null;
  parts: number;
  sizeBytes: number;
}
