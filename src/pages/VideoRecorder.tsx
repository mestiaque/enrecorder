import { Monitor } from "lucide-react";
import { EncoderTable } from "../components/media/EncoderTable";
import { PageHeader, Panel, Pending, PhaseNotice, Row, StatusIcon } from "../components/ui/blocks";
import { useSystem } from "../lib/queries";
import { captureBackendLabel } from "../lib/readiness";

const BACKEND_NOTE: Record<string, string> = {
  x11: "ENRecorder captures monitors, windows and regions directly on X11.",
  wayland: "On Wayland, your desktop asks which screen or window to share (a portal dialog) each time; ENRecorder never captures without it.",
  windows: "Windows.Graphics.Capture shows a yellow border around what is being recorded.",
  unknown: "No graphical session was found, so the screen cannot be captured.",
};

export function VideoRecorderPage() {
  const { data: sys } = useSystem();
  return (
    <>
      <PageHeader title="Video Recorder" subtitle="Record the desktop, a monitor, a window, a region or a webcam." />
      <PhaseNotice phase={2} title="Screen recording is being built">
        The native capture engine (Windows.Graphics.Capture, X11, Wayland portal + PipeWire) arrives in the next phase. Nothing is recorded yet; this page already shows what ENRecorder found on this
        computer.
      </PhaseNotice>
      <div className="grid gap-5 xl:grid-cols-2">
        <Panel title="Capture backend" bodyClass="py-1">
          {!sys ? (
            <Pending label="Detecting…" />
          ) : (
            <>
              <Row label="Display server">
                <span className="flex items-center gap-1.5">
                  <StatusIcon level={sys.displayServer === "unknown" ? "bad" : "ok"} className="h-3.5 w-3.5" />
                  {sys.displayServer === "x11" ? "X11" : sys.displayServer === "wayland" ? "Wayland" : sys.displayServer === "windows" ? "Windows" : "None"}
                  {sys.desktop && <span className="text-faint">· {sys.desktop}</span>}
                </span>
              </Row>
              <Row label="Capture method">{captureBackendLabel(sys)}</Row>
              {sys.os === "linux" && <Row label="PipeWire">{sys.pipewire ? "Running" : "Not running"}</Row>}
              <p className="py-2.5 text-[11px] text-faint">{BACKEND_NOTE[sys.displayServer]}</p>
            </>
          )}
        </Panel>
        <Panel title="Displays" bodyClass="py-1">
          {!sys ? (
            <Pending label="Detecting…" />
          ) : sys.monitors.length === 0 ? (
            <div className="py-3 text-[13px] text-muted">No display reported.</div>
          ) : (
            sys.monitors.map((m, i) => (
              <Row
                key={`${m.name}-${i}`}
                label={
                  <span className="flex items-center gap-2">
                    <Monitor className="h-3.5 w-3.5 text-faint" />
                    {m.name}
                    {m.primary && <span className="rounded bg-accent/15 px-1.5 text-[10px] font-semibold text-accent">Primary</span>}
                  </span>
                }
                hint={`Position ${m.x}, ${m.y} · scale ${Math.round(m.scale * 100)}%`}
              >
                <span className="tabular-nums">
                  {m.width}×{m.height}
                </span>
              </Row>
            ))
          )}
        </Panel>
        <div className="xl:col-span-2">
          <EncoderTable />
        </div>
      </div>
    </>
  );
}
