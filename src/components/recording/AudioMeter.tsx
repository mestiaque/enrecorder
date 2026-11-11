import { cn } from "../../lib/format";

/** Peak level meter: -60 dBFS (empty) … 0 dBFS (full); amber above -12, red above -3. */
export function AudioMeter({ level, className }: { level: number | null; className?: string }) {
  const db = level ?? -120;
  const pct = Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
  const tone = db > -3 ? "bg-danger" : db > -12 ? "bg-amber-500" : "bg-success";
  return (
    <div className={cn("flex items-center gap-3", className)}>
      <div className="relative h-2 flex-1 overflow-hidden rounded-full bg-line" role="meter" aria-label="Audio level" aria-valuemin={-60} aria-valuemax={0} aria-valuenow={Math.round(db)}>
        <div className={cn("h-full rounded-full transition-[width] duration-100", tone)} style={{ width: `${pct}%` }} />
      </div>
      <span className="w-16 text-right font-mono text-[11px] tabular-nums text-muted">{level == null || db <= -100 ? "silence" : `${db.toFixed(0)} dB`}</span>
    </div>
  );
}
