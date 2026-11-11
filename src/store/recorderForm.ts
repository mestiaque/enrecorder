/** What to record (the rest of the form lives in Settings) and how to start from anywhere. */
import { create } from "zustand";
import { api, errorMessage } from "../lib/api";
import type { AudioDevice, AudioOptions, CaptureSource, RecordingOptions, Settings } from "../lib/types";
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

/** A stored device choice → the device to open: "" off, "default" the system default. */
export function resolveDevice(choice: string, kind: AudioDevice["kind"], devices: AudioDevice[]): string | null {
  if (!choice) return null;
  if (choice !== "default") return choice;
  const list = devices.filter((d) => d.kind === kind);
  return (list.find((d) => d.default) ?? list[0])?.id ?? null;
}

export function audioFrom(mic: string, system: string, separate: boolean, devices: AudioDevice[], s?: Settings): AudioOptions {
  return {
    microphone: resolveDevice(mic, "microphone", devices),
    system: resolveDevice(system, "system", devices),
    separateTracks: separate,
    micVolume: s?.recMicVolume ?? 100,
    systemVolume: s?.recSystemVolume ?? 100,
    micDenoise: s?.recMicDenoise ?? false,
    micNormalize: s?.recMicNormalize ?? false,
    micGate: s?.recMicGate ?? false,
    echoCancel: s?.recEchoCancel ?? false,
    sampleRate: s?.audioSampleRate ?? 48000,
    channels: s?.audioChannels ?? 2,
  };
}

let deviceCache: AudioDevice[] = [];
async function devices(): Promise<AudioDevice[]> {
  try {
    deviceCache = await api.listAudioDevices();
  } catch {
    /* keep the last list */
  }
  return deviceCache;
}

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
    extras: { splitMinutes: s.recSplitMinutes, softwareOnly: !s.hwAccel, trackClicks: s.recTrackClicks, trackKeys: s.recTrackKeys },
  };
}

/** The full recording request from Settings and the chosen source (audio, webcam, extras). */
export async function buildOptions(source: CaptureSource): Promise<RecordingOptions> {
  const s = useSettings.getState().settings;
  const opts = optionsFrom(s, source);
  if (s.recMic || s.recSystemAudio) opts.audio = audioFrom(s.recMic, s.recSystemAudio, s.recSeparateTracks, await devices(), s);
  if (s.recWebcam) opts.webcam = { device: s.recWebcam, position: s.recWebcamPosition, size: s.recWebcamSize, shape: s.recWebcamShape, border: s.recWebcamBorder, shadow: s.recWebcamShadow };
  return opts;
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
    await api.startRecording(await buildOptions(source));
    return true;
  } catch (e) {
    useNav.getState().go("video");
    toast({ kind: "error", title: "Could not start recording", detail: errorMessage(e) }, 10000);
    return false;
  }
}
