/** The current recording, kept in sync with the backend's recording events. */
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { api, errorMessage, EVENTS } from "../lib/api";
import { queryClient } from "../lib/queryClient";
import type { RecState, RecordingSnapshot } from "../lib/types";
import { toast } from "./ui";

export const ACTIVE: RecState[] = ["preparing", "countdown", "recording", "paused", "finalizing"];
export const isActive = (s: RecordingSnapshot | null | undefined) => !!s && ACTIVE.includes(s.state);

interface RecordingStore {
  snap: RecordingSnapshot | null;
  init(): Promise<() => void>;
}

function finished(s: RecordingSnapshot) {
  void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
  void queryClient.invalidateQueries({ queryKey: ["media"] });
  if (s.state === "completed" && s.outputPath) {
    const path = s.outputPath;
    toast(
      { kind: "success", title: "Recording saved", detail: s.notice ? `${s.notice}.\n${path}` : path, action: { label: "Show in folder", run: () => void api.revealPath(path).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) })) } },
      10000,
    );
  } else if (s.state === "failed") {
    toast({ kind: "error", title: "Recording failed", detail: s.error ?? undefined }, 15000);
  }
}

export const useRecording = create<RecordingStore>((set, get) => ({
  snap: null,
  async init() {
    set({ snap: await api.recordingStatus().catch(() => null) });
    const unState = listen<RecordingSnapshot>(EVENTS.recordingState, (e) => {
      const prev = get().snap;
      set({ snap: e.payload });
      const done = ["completed", "failed", "cancelled"].includes(e.payload.state);
      if (done && prev?.state !== e.payload.state) finished(e.payload);
    });
    const unProgress = listen<RecordingSnapshot>(EVENTS.recordingProgress, (e) => {
      const cur = get().snap;
      // Progress never moves the state backwards (events can arrive out of order).
      if (!cur || cur.sessionId !== e.payload.sessionId || cur.state === e.payload.state) set({ snap: e.payload });
    });
    return () => {
      void unState.then((f) => f());
      void unProgress.then((f) => f());
    };
  },
}));
