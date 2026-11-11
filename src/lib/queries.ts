/** TanStack Query hooks for backend data. Mutations invalidate the matching keys. */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { api, EVENTS } from "./api";
import type { LibraryQuery } from "./types";

export const keys = {
  system: ["system"] as const,
  tools: ["tools"] as const,
  dashboard: ["dashboard"] as const,
  media: (q: LibraryQuery) => ["media", q] as const,
  exports: ["exports"] as const,
  paths: ["paths"] as const,
};

/** OS, display server, audio server, monitors. Refreshed when the window regains focus. */
export const useSystem = () => useQuery({ queryKey: keys.system, queryFn: api.systemInfo, staleTime: 30_000 });

/** FFmpeg status and verified encoders; updated live by the backend's `tools-changed` event. */
export function useTools() {
  const qc = useQueryClient();
  useEffect(() => {
    const un = listen(EVENTS.toolsChanged, () => void qc.invalidateQueries({ queryKey: keys.tools }));
    return () => void un.then((f) => f());
  }, [qc]);
  return useQuery({ queryKey: keys.tools, queryFn: api.mediaTools, staleTime: Infinity });
}

export const useDashboard = () => useQuery({ queryKey: keys.dashboard, queryFn: api.dashboard, staleTime: 15_000 });
export const useMedia = (q: LibraryQuery) => useQuery({ queryKey: keys.media(q), queryFn: () => api.listMedia(q), placeholderData: (prev) => prev });
export const useExportJobs = () => useQuery({ queryKey: keys.exports, queryFn: api.listExportJobs });
export const usePaths = () => useQuery({ queryKey: keys.paths, queryFn: api.appPaths });

/** Microphones and system-audio sources; refreshed when the page is shown again. */
export const useAudioDevices = () => useQuery({ queryKey: ["audio-devices"], queryFn: api.listAudioDevices, staleTime: 10_000 });
