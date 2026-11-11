/** The scheduled recording (one at a time) and the screenshot-shortcut request. */
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { api, EVENTS } from "../lib/api";
import type { ScheduleInfo } from "../lib/types";

interface ScheduleStore {
  info: ScheduleInfo | null;
  init(): Promise<() => void>;
}

export const useSchedule = create<ScheduleStore>((set) => ({
  info: null,
  async init() {
    set({ info: await api.scheduleStatus().catch(() => null) });
    const un = listen<ScheduleInfo | null>(EVENTS.scheduleChanged, (e) => set({ info: e.payload }));
    return () => void un.then((f) => f());
  },
}));

/** Bumped by the screenshot shortcut; the Screenshot page takes one when it sees a new value. */
export const useShotRequest = create<{ n: number }>(() => ({ n: 0 }));
