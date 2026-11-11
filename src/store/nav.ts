import { create } from "zustand";

export type PageId = "home" | "video" | "audio" | "screenshot" | "editor" | "recordings" | "exports";

export const PAGE_IDS: PageId[] = ["home", "video", "audio", "screenshot", "editor", "recordings", "exports"];

const KEY = "enrecorder.page";

function initial(): PageId {
  try {
    const v = localStorage.getItem(KEY) as PageId | null;
    return v && PAGE_IDS.includes(v) ? v : "home";
  } catch {
    return "home";
  }
}

interface NavStore {
  page: PageId;
  go(p: PageId): void;
}

/** The current page, remembered between starts. */
export const useNav = create<NavStore>((set) => ({
  page: initial(),
  go(page) {
    try {
      localStorage.setItem(KEY, page);
    } catch {
      /* storage unavailable: only for this session */
    }
    set({ page });
  },
}));
