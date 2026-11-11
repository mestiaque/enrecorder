/** What to record (the rest of the form lives in Settings) and how to start from anywhere. */
import { create } from "zustand";
import { api, errorMessage } from "../lib/api";
import type { CaptureSource, RecordingOptions, Settings } from "../lib/types";
import { useNav } from "./nav";
import { useSettings } from "./settings";
import { toast } from "./ui";

interface FormStore {
  source: CaptureSource | null;
  setSource(s: CaptureSource | null): void;
}

export const useRecorderForm = create<FormStore>((set) => ({
  source: null,
  setSource: (source) => set({ source }),
}));

export function optionsFrom(s: Settings, source: CaptureSource): RecordingOptions {
  return {
    source,
    fps: s.recFps,
    resolution: s.recResolution,
    format: s.recFormat,
    codec: s.recCodec,
    encoder: s.recEncoder,
    quality: s.recQuality,
    showCursor: s.recCursor,
    countdown: s.recCountdown,
    maxMinutes: s.recMaxMinutes,
  };
}

/** Starts with the current form (Record button, Ctrl+Shift+R, tray). */
export async function startRecording(): Promise<boolean> {
  const source = useRecorderForm.getState().source;
  if (!source) {
    useNav.getState().go("video");
    toast({ kind: "info", title: "Choose what to record first" });
    return false;
  }
  try {
    await api.startRecording(optionsFrom(useSettings.getState().settings, source));
    return true;
  } catch (e) {
    useNav.getState().go("video");
    toast({ kind: "error", title: "Could not start recording", detail: errorMessage(e) }, 10000);
    return false;
  }
}
