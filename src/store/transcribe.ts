/** The running transcription (one at a time) and its progress, shared by every details panel. */
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { api, errorMessage, EVENTS } from "../lib/api";
import { toast, useUi } from "./ui";

interface TranscribeStore {
  mediaId: string | null;
  percent: number;
  start(id: string, title: string): Promise<void>;
}

export const useTranscribe = create<TranscribeStore>((set, get) => ({
  mediaId: null,
  percent: 0,
  async start(id, title) {
    if (get().mediaId) return;
    const st = await api.transcriptionStatus().catch(() => null);
    if (!st?.program || st.models.length === 0) {
      toast(
        {
          kind: "error",
          title: "Transcription is not set up",
          detail: !st?.program ? "Install whisper.cpp (whisper-cli) or choose it in Settings → Tools." : "Add a whisper.cpp speech model (ggml-*.bin) in Settings → Tools.",
          action: { label: "Open Settings", run: () => useUi.getState().openDialog("settings", "tools") },
        },
        8000,
      );
      return;
    }
    set({ mediaId: id, percent: 0 });
    const un = await listen<{ mediaId: string; percent: number }>(EVENTS.transcribeProgress, (e) => {
      if (e.payload.mediaId === id) set({ percent: e.payload.percent });
    });
    try {
      const srt = await api.transcribeMedia(id);
      toast({ kind: "success", title: "Transcript saved", detail: srt.split(/[\\/]/).pop(), action: { label: "Show", run: () => void api.revealPath(srt) } }, 8000);
    } catch (e) {
      const msg = errorMessage(e);
      if (msg !== "Cancelled") toast({ kind: "error", title: `Could not transcribe "${title}"`, detail: msg }, 8000);
    } finally {
      un();
      set({ mediaId: null, percent: 0 });
    }
  },
}));
