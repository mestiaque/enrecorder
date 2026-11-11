import { useQuery } from "@tanstack/react-query";
import { CalendarClock, Circle, Monitor, X } from "lucide-react";
import { useState } from "react";
import { Dialog } from "../components/ui/overlays";
import { useSchedule } from "../store/schedule";
import { errorMessage } from "../lib/api";
import { toast } from "../store/ui";
import { EncoderTable } from "../components/media/EncoderTable";
import { LivePanel } from "../components/recording/LivePanel";
import { RecordSettings, verifiedFor } from "../components/recording/RecordSettings";
import { SourcePicker, sourceLabel } from "../components/recording/SourcePicker";
import { PageHeader, Panel, Pending, Row, StatusIcon } from "../components/ui/blocks";
import { Button, Kbd } from "../components/ui/primitives";
import { api } from "../lib/api";
import { useSystem, useTools } from "../lib/queries";
import { captureBackendLabel } from "../lib/readiness";
import { buildOptions, useRecorderForm, startRecording } from "../store/recorderForm";
import { isActive, useRecording } from "../store/recording";
import { useSettings } from "../store/settings";

const BACKEND_NOTE: Record<string, string> = {
  x11: "ENRecorder captures monitors, windows and areas directly from the X server.",
  wayland: "On Wayland, your desktop asks which screen or window to share each time; ENRecorder never captures without it.",
  windows: "ENRecorder records monitors and areas with Desktop Duplication (sharp at any display scaling) and windows with GDI.",
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

function ScheduleDialog({ onClose }: { onClose: () => void }) {
  const source = useRecorderForm((s) => s.source);
  const pad = (n: number) => String(n).padStart(2, "0");
  const soon = new Date(Date.now() + 10 * 60_000);
  const [when, setWhen] = useState(`${soon.getFullYear()}-${pad(soon.getMonth() + 1)}-${pad(soon.getDate())}T${pad(soon.getHours())}:${pad(soon.getMinutes())}`);
  const save = async () => {
    if (!source) return;
    const at = new Date(when).getTime();
    try {
      await api.scheduleRecording(await buildOptions(source), at, sourceLabel(source));
      toast({ kind: "success", title: "Recording scheduled", detail: "Keep ENRecorder running (it can stay in the tray)." });
      onClose();
    } catch (e) {
      toast({ kind: "error", title: "Could not schedule", detail: errorMessage(e) });
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title="Schedule a recording"
      description="It starts by itself with the current settings, even with the window hidden. It stops by the time in “Stop automatically”, or when you stop it."
      className="w-[min(440px,94vw)]"
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={() => void save()}>
            Schedule
          </Button>
        </>
      }
    >
      <label className="block text-[12px] text-muted">
        Start at
        <input type="datetime-local" value={when} onChange={(e) => setWhen(e.target.value)} className="mt-1 h-9 w-full rounded-md border border-line bg-bg px-2 text-[13px] text-fg outline-none focus:border-accent" />
      </label>
      <div className="mt-2 text-[11px] text-faint">{source ? sourceLabel(source) : ""}</div>
    </Dialog>
  );
}

export function VideoRecorderPage() {
  const [scheduling, setScheduling] = useState(false);
  const schedule = useSchedule((s) => s.info);
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
              <Button className="h-11" disabled={!!blocker || !source || source.type === "portal"} title={source?.type === "portal" ? "On Wayland the sharing dialog needs you, so recordings cannot start by themselves" : "Start at a chosen time"} onClick={() => setScheduling(true)}>
                <CalendarClock className="h-4 w-4" />
                Schedule…
              </Button>
              <div className="min-w-0 text-[12px] text-muted">
                {blocker ?? (source ? sourceLabel(source) : "")}
                <div className="text-[11px] text-faint">
                  <Kbd>Ctrl+Shift+R</Kbd> starts and stops from anywhere · microphone and system audio under Recording settings
                </div>
              </div>
              {schedule && (
                <div className="flex w-full items-center gap-2 rounded-lg border border-accent/40 bg-accent/10 px-3 py-2 text-[12px]">
                  <CalendarClock className="h-4 w-4 text-accent" />
                  <span className="flex-1">
                    Starts {new Date(schedule.atMs).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })} · {schedule.label}
                  </span>
                  <Button size="sm" variant="ghost" onClick={() => void api.cancelSchedule()}>
                    <X className="h-3.5 w-3.5" />
                    Cancel
                  </Button>
                </div>
              )}
            </div>
            <SystemPanel />
          </div>
          <div className="min-w-0">
            <RecordSettings backend={sources.data?.backend} />
          </div>
          <div className="xl:col-span-2">
            <EncoderTable />
          </div>
          {scheduling && <ScheduleDialog onClose={() => setScheduling(false)} />}
        </div>
      )}
    </>
  );
}
