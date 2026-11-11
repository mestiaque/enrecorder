import { useQuery } from "@tanstack/react-query";
import { Circle, Monitor } from "lucide-react";
import { EncoderTable } from "../components/media/EncoderTable";
import { LivePanel } from "../components/recording/LivePanel";
import { RecordSettings, verifiedFor } from "../components/recording/RecordSettings";
import { SourcePicker, sourceLabel } from "../components/recording/SourcePicker";
import { PageHeader, Panel, Pending, Row, StatusIcon } from "../components/ui/blocks";
import { Button, Kbd } from "../components/ui/primitives";
import { api } from "../lib/api";
import { useSystem, useTools } from "../lib/queries";
import { captureBackendLabel } from "../lib/readiness";
import { useRecorderForm, startRecording } from "../store/recorderForm";
import { isActive, useRecording } from "../store/recording";
import { useSettings } from "../store/settings";

const BACKEND_NOTE: Record<string, string> = {
  x11: "ENRecorder captures monitors, windows and areas directly from the X server.",
  wayland: "On Wayland, your desktop asks which screen or window to share each time; ENRecorder never captures without it.",
  windows: "Windows.Graphics.Capture shows a border around what is being recorded.",
  unknown: "No graphical session was found, so the screen cannot be captured.",
};

function SystemPanel() {
  const { data: sys } = useSystem();
  return (
    <Panel title="This computer" bodyClass="py-1">
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
          {sys.monitors.map((m, i) => (
            <Row
              key={`${m.name}-${i}`}
              label={
                <span className="flex items-center gap-2">
                  <Monitor className="h-3.5 w-3.5 text-faint" />
                  {m.name}
                  {m.primary && <span className="rounded bg-accent/15 px-1.5 text-[10px] font-semibold text-accent">Primary</span>}
                </span>
              }
              hint={`scale ${Math.round(m.scale * 100)}%`}
            >
              <span className="tabular-nums">
                {m.width}×{m.height}
              </span>
            </Row>
          ))}
          <p className="py-2.5 text-[11px] text-faint">{BACKEND_NOTE[sys.displayServer]}</p>
        </>
      )}
    </Panel>
  );
}

export function VideoRecorderPage() {
  const snap = useRecording((s) => s.snap);
  const source = useRecorderForm((s) => s.source);
  const codec = useSettings((s) => s.settings.recCodec);
  const { data: tools } = useTools();
  const sources = useQuery({ queryKey: ["capture-sources"], queryFn: api.listCaptureSources, staleTime: 5000 });
  const active = isActive(snap);
  const encoders = tools?.capabilities?.encoders;
  const blocker = !tools
    ? "Loading…"
    : !tools.ffmpeg
      ? "FFmpeg was not found (Settings → Tools)"
      : !encoders
        ? "Testing encoders…"
        : !verifiedFor(encoders, codec).length
          ? "No encoder for this codec passed its test"
          : sources.data?.unsupported
            ? "Recording is not available on this system yet"
            : !source
              ? "Choose what to record"
              : null;

  return (
    <>
      <PageHeader title="Video Recorder" subtitle="Record the desktop, a monitor, a window or an area of the screen." />
      {active && snap ? (
        <div className="space-y-5">
          <LivePanel snap={snap} />
          {source && <div className="text-[12px] text-muted">Recording: {sourceLabel(source)}</div>}
        </div>
      ) : (
        <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_420px]">
          <div className="min-w-0 space-y-5">
            <SourcePicker />
            <div className="flex flex-wrap items-center gap-3 rounded-xl border border-line bg-panel p-4">
              <Button variant="primary" className="h-11 px-5 text-[14px]" disabled={!!blocker} onClick={() => void startRecording()}>
                <Circle className="h-4 w-4 fill-current text-white" />
                Start recording
              </Button>
              <div className="min-w-0 text-[12px] text-muted">
                {blocker ?? (source ? sourceLabel(source) : "")}
                <div className="text-[11px] text-faint">
                  <Kbd>Ctrl+Shift+R</Kbd> starts and stops from anywhere · video only for now (audio arrives in phase 3)
                </div>
              </div>
            </div>
            <SystemPanel />
          </div>
          <div className="min-w-0">
            <RecordSettings backend={sources.data?.backend} />
          </div>
          <div className="xl:col-span-2">
            <EncoderTable />
          </div>
        </div>
      )}
    </>
  );
}
