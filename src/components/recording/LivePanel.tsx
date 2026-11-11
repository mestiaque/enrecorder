import { Pause, Play, Square } from "lucide-react";
import { api, errorMessage } from "../../lib/api";
import { cn, formatBytes, formatDuration } from "../../lib/format";
import type { RecordingSnapshot } from "../../lib/types";
import { toast } from "../../store/ui";
import { Button, Kbd } from "../ui/primitives";
import { AudioMeter } from "./AudioMeter";

const STATE: Record<string, string> = { preparing: "Starting…", countdown: "Starting…", recording: "Recording", paused: "Paused", finalizing: "Saving…" };

const act = (f: () => Promise<void>, what: string) => f().catch((e) => toast({ kind: "error", title: `Could not ${what}`, detail: errorMessage(e) }));

export function LivePanel({ snap }: { snap: RecordingSnapshot }) {
  const rec = snap.state === "recording";
  const paused = snap.state === "paused";
  return (
    <section className="rounded-xl border border-line bg-panel p-6">
      <div className="flex flex-wrap items-center gap-6">
        <div className="flex items-center gap-3">
          <span className={cn("h-3.5 w-3.5 rounded-full", rec ? "animate-pulse bg-danger" : paused ? "bg-amber-500" : "bg-faint")} />
          <span className="text-[13px] font-medium">{STATE[snap.state] ?? snap.state}</span>
        </div>
        <div className="font-mono text-[44px] font-semibold leading-none tabular-nums">{formatDuration(snap.elapsedMs)}</div>
        <div className="ml-auto flex gap-2">
          {(rec || paused) && (
            <Button className="h-10 px-4" onClick={() => void act(rec ? api.pauseRecording : api.resumeRecording, rec ? "pause" : "resume")}>
              {rec ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
              {rec ? "Pause" : "Resume"}
            </Button>
          )}
          <Button variant="danger" className="h-10 px-4" disabled={snap.state === "finalizing"} onClick={() => void act(api.stopRecording, "stop")}>
            <Square className="h-4 w-4 fill-current" />
            {snap.state === "finalizing" ? "Saving…" : "Stop"}
          </Button>
        </div>
      </div>
      {snap.audioTracks > 0 && <AudioMeter level={rec ? snap.audioLevel : null} className="mt-5" />}
      {snap.audioOnly ? (
        <div className="mt-5 grid grid-cols-2 gap-4 border-t border-line pt-4 text-[12px] sm:grid-cols-3">
          <Info label="Format" value={snap.encoder} />
          <Info label="File so far" value={formatBytes(snap.sizeBytes)} />
          <Info label="Tracks" value={String(snap.audioTracks)} />
        </div>
      ) : (
        <div className="mt-5 grid grid-cols-2 gap-4 border-t border-line pt-4 text-[12px] sm:grid-cols-6">
          <Info label="Size" value={`${snap.width}×${snap.height}`} />
          <Info label="Encoder" value={snap.encoder} />
          <Info label="File so far" value={formatBytes(snap.sizeBytes)} />
          <Info label="Frame rate" value={snap.fps ? `${snap.fps.toFixed(1)} fps` : "—"} />
          <Info label="Dropped frames" value={String(snap.dropped)} warn={snap.dropped > 0} />
          <Info label="Audio" value={snap.audioTracks ? `${snap.audioTracks} track${snap.audioTracks > 1 ? "s" : ""}` : "Off"} />
        </div>
      )}
      <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-faint">
        {!snap.audioOnly && (
          <span>
            <Kbd>Ctrl+Shift+R</Kbd> stop
          </span>
        )}
        <span>
          <Kbd>Ctrl+Shift+P</Kbd> pause / resume
        </span>
        <span>The tray icon is red while recording.</span>
      </div>
    </section>
  );
}

function Info({ label, value, warn }: { label: string; value: string; warn?: boolean }) {
  return (
    <div className="min-w-0">
      <div className="text-faint">{label}</div>
      <div className={cn("mt-0.5 truncate font-medium tabular-nums", warn && "text-amber-500")}>{value}</div>
    </div>
  );
}

/** Big 3-2-1 over the window before recording starts. */
export function CountdownOverlay({ snap }: { snap: RecordingSnapshot }) {
  return (
    <div className="fixed inset-0 z-[70] flex flex-col items-center justify-center bg-bg/80 backdrop-blur-sm animate-fade-in">
      <div key={snap.countdownLeft} className="text-[120px] font-semibold leading-none tabular-nums animate-scale-in">
        {snap.countdownLeft || ""}
      </div>
      <div className="mt-4 text-[13px] text-muted">Recording starts…</div>
      <Button className="mt-6" onClick={() => void act(api.stopRecording, "cancel")}>
        Cancel
      </Button>
    </div>
  );
}
