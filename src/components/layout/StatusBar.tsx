import { HardDrive } from "lucide-react";
import { formatBytes } from "../../lib/format";
import { useDashboard, useSystem, useTools } from "../../lib/queries";
import { shortVersion } from "../../lib/readiness";

const DISPLAY: Record<string, string> = { x11: "X11", wayland: "Wayland", windows: "Windows", unknown: "No display" };

export function StatusBar() {
  const { data: tools } = useTools();
  const { data: sys } = useSystem();
  const { data: dash } = useDashboard();
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
      {dash?.disk && (
        <span className="flex items-center gap-1 tabular-nums">
          <HardDrive className="h-3 w-3" />
          {formatBytes(dash.disk.available)} free
        </span>
      )}
    </footer>
  );
}
