/** Floating recording controls (separate always-on-top window). */
import { getCurrentWindow } from "@tauri-apps/api/window";
import { GripVertical, Pause, Play, Square } from "lucide-react";
import { useEffect } from "react";
import { api } from "../lib/api";
import { cn, formatDuration } from "../lib/format";
import { useRecording } from "../store/recording";

export function HudView() {
  const snap = useRecording((s) => s.snap);
  useEffect(() => {
    let off: (() => void) | undefined;
    void useRecording.getState().init().then((f) => (off = f));
    return () => off?.();
  }, []);
  const rec = snap?.state === "recording";
  const paused = snap?.state === "paused";
  const btn = "flex h-8 w-8 items-center justify-center rounded-full text-white/85 transition-colors hover:bg-white/15 hover:text-white";
  return (
    <div className="flex h-full items-center justify-center p-1">
      <div className="flex h-11 w-full items-center gap-2 rounded-full border border-white/15 bg-[#18181b]/95 pl-1 pr-1.5 text-white shadow-lg">
        <button className="flex h-8 w-5 cursor-grab items-center justify-center text-white/40" onMouseDown={() => void getCurrentWindow().startDragging()} aria-label="Move">
          <GripVertical className="h-4 w-4" />
        </button>
        <span className={cn("h-3 w-3 shrink-0 rounded-full", rec ? "animate-pulse bg-red-500" : paused ? "bg-amber-400" : "bg-white/40")} />
        <span className="min-w-[72px] font-mono text-[15px] font-semibold tabular-nums">{formatDuration(snap?.elapsedMs ?? 0)}</span>
        <span className="flex-1 truncate text-[11px] text-white/60">{rec ? "Recording" : paused ? "Paused" : snap?.state === "finalizing" ? "Saving…" : ""}</span>
        {/* Always rendered so the buttons never shift under the pointer. */}
        <button
          className={cn(btn, "disabled:opacity-30 disabled:hover:bg-transparent")}
          disabled={!rec && !paused}
          title={paused ? "Resume (Ctrl+Shift+P)" : "Pause (Ctrl+Shift+P)"}
          onClick={() => void (rec ? api.pauseRecording() : api.resumeRecording())}
        >
          {paused ? <Play className="h-4 w-4" /> : <Pause className="h-4 w-4" />}
        </button>
        <button className={cn(btn, "bg-red-500/90 text-white hover:bg-red-500")} title="Stop (Ctrl+Shift+R)" onClick={() => void api.stopRecording()}>
          <Square className="h-3.5 w-3.5 fill-current" />
        </button>
      </div>
    </div>
  );
}
