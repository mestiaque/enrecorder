import { HardDrive } from "lucide-react";
import { formatBytes, formatDuration } from "../../lib/format";
import { isActive, useRecording } from "../../store/recording";
import { useSchedule } from "../../store/schedule";
import { useDashboard, useSystem, useTools } from "../../lib/queries";
import { shortVersion } from "../../lib/readiness";

const DISPLAY: Record<string, string> = { x11: "X11", wayland: "Wayland", windows: "Windows", unknown: "No display" };

export function StatusBar() {
  const { data: tools } = useTools();
  const { data: sys } = useSystem();
  const { data: dash } = useDashboard();
  const rec = useRecording((s) => s.snap);
  const schedule = useSchedule((s) => s.info);
  const verified = tools?.capabilities?.encoders.filter((e) => e.verified).length;
  const ffmpeg = !tools
    ? "…"
    : tools.ffmpeg
      ? `FFmpeg ${shortVersion(tools.ffmpeg.version)}${tools.checking ? " · testing encoders…" : verified != null ? ` · ${verified} encoder${verified === 1 ? "" : "s"} verified` : ""}`
      : tools.checking
        ? "Looking for FFmpeg…"
        : "FFmpeg not found";
  return (
    <footer className="flex h-7 shrink-0 items-center gap-4 border-t border-line bg-panel px-3 text-[11px] text-muted">
      <span className={tools && !tools.ffmpeg && !tools.checking ? "text-danger" : undefined}>{ffmpeg}</span>
      {sys && (
        <span>
          {DISPLAY[sys.displayServer]}
          {sys.desktop ? ` · ${sys.desktop}` : ""}
        </span>
      )}
      <span className="flex-1" />
      {schedule && !isActive(rec) && <span className="text-accent">Scheduled {new Date(schedule.atMs).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}</span>}
      {isActive(rec) && rec && (
        <span className="flex items-center gap-1.5 font-medium text-danger">
          <span className="h-2 w-2 animate-pulse rounded-full bg-danger" />
          {rec.state === "paused" ? "Paused" : rec.state === "finalizing" ? "Saving" : "REC"} {formatDuration(rec.elapsedMs)}
        </span>
      )}
      {dash?.disk && (
        <span className="flex items-center gap-1 tabular-nums">
          <HardDrive className="h-3 w-3" />
          {formatBytes(dash.disk.available)} free
        </span>
      )}
    </footer>
  );
}
