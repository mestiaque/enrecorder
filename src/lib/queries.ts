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
/** Export jobs; refreshed live by `export-progress` events (progress patched in place). */
export function useExportJobs() {
  const qc = useQueryClient();
  useEffect(() => {
    const un = listen<import("./types").ExportUpdate>(EVENTS.exportProgress, (e) => {
      const u = e.payload;
      let known = false;
      qc.setQueryData<import("./types").ExportJob[]>(keys.exports, (list) =>
        list?.map((j) => {
          if (j.id !== u.id) return j;
          known = true;
          return { ...j, status: u.status, progress: u.progress, error: u.error };
        }),
      );
      if (!known || ["completed", "failed", "cancelled"].includes(u.status)) void qc.invalidateQueries({ queryKey: keys.exports });
      if (u.status === "completed") void qc.invalidateQueries({ queryKey: ["media"] });
    });
    return () => void un.then((f) => f());
  }, [qc]);
  return useQuery({ queryKey: keys.exports, queryFn: api.listExportJobs });
}

/** Thumbnail as an asset URL (made by the backend on first use). */
export const useThumbnail = (id: string) => useQuery({ queryKey: ["thumb", id], queryFn: () => api.mediaThumbnail(id), staleTime: Infinity, retry: false });
export const usePaths = () => useQuery({ queryKey: keys.paths, queryFn: api.appPaths });

/** Microphones and system-audio sources; refreshed when the page is shown again. */
export const useAudioDevices = () => useQuery({ queryKey: ["audio-devices"], queryFn: api.listAudioDevices, staleTime: 10_000 });

/** Where the preview player streams a recording from (local preview server). */
export const useMediaUrl = (id: string | null) => useQuery({ queryKey: ["media-url", id], queryFn: () => api.mediaUrl(id!), enabled: !!id, staleTime: Infinity });

export const useCameras = () => useQuery({ queryKey: ["cameras"], queryFn: api.listCameras, staleTime: 10_000 });
