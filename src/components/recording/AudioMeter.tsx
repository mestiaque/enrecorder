import { AlertTriangle } from "lucide-react";
import { useEffect, useRef } from "react";
import { cn } from "../../lib/format";

const pct = (db: number | null | undefined) => Math.max(0, Math.min(100, (((db ?? -120) + 60) / 60) * 100));

/**
 * Level meter: peak (bar) and RMS (darker inner bar) from -60 dBFS (empty) to 0 dBFS (full),
 * a short level history (waveform) and a clipping warning.
 */
export function AudioMeter({ level, rms, clipped = 0, className }: { level: number | null; rms?: number | null; clipped?: number; className?: string }) {
  const db = level ?? -120;
  const tone = db > -3 ? "bg-danger" : db > -12 ? "bg-amber-500" : "bg-success";
  const canvas = useRef<HTMLCanvasElement>(null);
  const history = useRef<number[]>([]);
  // Waveform: one column per update, newest on the right.
  useEffect(() => {
    const h = history.current;
    h.push(level == null ? 0 : pct(level) / 100);
    if (h.length > 120) h.shift();
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const { width: w, height: ht } = c;
    ctx.clearRect(0, 0, w, ht);
    const col = w / 120;
    const style = getComputedStyle(document.documentElement);
    ctx.fillStyle = `rgb(${style.getPropertyValue("--accent").trim().replace(/ /g, ",")})`;
    h.forEach((v, i) => {
      const bar = Math.max(1, v * ht);
      ctx.fillRect(w - (h.length - i) * col, (ht - bar) / 2, Math.max(1, col - 1), bar);
    });
  }, [level]);
  return (
    <div className={cn("space-y-1.5", className)}>
      <canvas ref={canvas} width={480} height={36} className="h-9 w-full" aria-hidden="true" />
      <div className="flex items-center gap-3">
        <div className="relative h-2 flex-1 overflow-hidden rounded-full bg-line" role="meter" aria-label="Audio level" aria-valuemin={-60} aria-valuemax={0} aria-valuenow={Math.round(db)}>
          <div className={cn("h-full rounded-full transition-[width] duration-100", tone)} style={{ width: `${pct(level)}%` }} />
          {rms != null && <div className="absolute inset-y-0 left-0 rounded-full bg-black/25 transition-[width] duration-100" style={{ width: `${pct(rms)}%` }} />}
        </div>
        <span className="w-28 text-right font-mono text-[11px] tabular-nums text-muted">
          {level == null || db <= -100 ? "silence" : `${db.toFixed(0)} dB${rms != null && rms > -100 ? ` · RMS ${rms.toFixed(0)}` : ""}`}
        </span>
      </div>
      {clipped > 0 && (
        <div className="flex items-center gap-1.5 text-[11px] text-danger">
          <AlertTriangle className="h-3.5 w-3.5" />
          The sound was cut off {clipped} time{clipped === 1 ? "" : "s"} (too loud) — lower the input volume.
        </div>
      )}
    </div>
  );
}
