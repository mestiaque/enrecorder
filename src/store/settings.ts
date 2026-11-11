import { create } from "zustand";
import { api, errorMessage } from "../lib/api";
import type { Settings, SettingsPayload } from "../lib/types";
import { toast } from "./ui";

/** Used until the backend answers (the backend owns the real defaults and validation). */
export const DEFAULT_SETTINGS: Settings = {
  startWithOs: false,
  startMinimized: false,
  closeToTray: true,
  language: "en",
  recordingsDir: "",
  screenshotsDir: "",
  ffmpegPath: "",
  recFps: 30,
  recResolution: "native",
  recFormat: "mp4",
  recCodec: "h264",
  recEncoder: "auto",
  recQuality: "balanced",
  recCursor: true,
  recCountdown: 3,
  recMaxMinutes: 0,
  recHideWindow: true,
  recShowHud: true,
  recMic: "",
  recSystemAudio: "",
  recSeparateTracks: false,
  checkUpdates: true,
  lastUpdateCheck: 0,
  recMicDenoise: false,
  recMicNormalize: false,
  recWebcam: "",
  recWebcamPosition: "bottom-right",
  recWebcamSize: 22,
  recWebcamShape: "circle",
  audioMic: "default",
  audioSystem: "",
  audioFormat: "mp3",
  audioQuality: "balanced",
  recWebcamBorder: "",
  recWebcamShadow: false,
  recMicGate: false,
  recEchoCancel: false,
  recMicVolume: 100,
  recSystemVolume: 100,
  audioSampleRate: 48000,
  audioChannels: 2,
  recSplitMinutes: 0,
  recTrackClicks: false,
  recTrackKeys: false,
  shotFormat: "png",
  shotQuality: 90,
  shotNameTemplate: "Screenshot {date} {time}",
  hwAccel: true,
  thumbnails: true,
  logRetentionDays: 14,
  notifyOnSave: true,
  whisperPath: "",
  whisperModel: "",
  transcribeLanguage: "auto",
  theme: "dark",
  accent: "#6366f1",
  animations: true,
  sidebarCollapsed: false,
};

interface SettingsStore {
  settings: Settings;
  defaultRecordings: string;
  defaultScreenshots: string;
  loaded: boolean;
  load(): Promise<void>;
  /** Optimistic update; rolled back with a toast when the backend refuses the value. */
  set<K extends keyof Settings>(key: K, value: Settings[K]): Promise<boolean>;
  apply(p: SettingsPayload): void;
}

export const useSettings = create<SettingsStore>((set, get) => ({
  settings: DEFAULT_SETTINGS,
  defaultRecordings: "",
  defaultScreenshots: "",
  loaded: false,
  async load() {
    try {
      get().apply(await api.getSettings());
    } catch (e) {
      console.error("settings load failed", e);
    }
    set({ loaded: true });
  },
  async set(key, value) {
    const prev = get().settings;
    if (prev[key] === value) return true;
    set({ settings: { ...prev, [key]: value } });
    try {
      set({ settings: await api.setSetting(key, value) });
      return true;
    } catch (e) {
      set({ settings: prev });
      toast({ kind: "error", title: "Could not save the setting", detail: errorMessage(e) });
      return false;
    }
  },
  apply(p) {
    set({ settings: p.settings, defaultRecordings: p.defaultRecordings, defaultScreenshots: p.defaultScreenshots });
  },
}));
