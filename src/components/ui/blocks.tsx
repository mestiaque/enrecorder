/** Page building blocks shared by every page (headers, panels, empty states, status dots). */
import { CheckCircle2, CircleAlert, CircleDashed, Info, XCircle } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "../../lib/format";
import type { CheckLevel } from "../../lib/readiness";
import { Spinner } from "./primitives";

export function PageHeader({ title, subtitle, actions }: { title: string; subtitle?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
      <div className="min-w-0">
        <h1 className="text-[20px] font-semibold tracking-tight">{title}</h1>
        {subtitle && <p className="mt-0.5 text-[13px] text-muted">{subtitle}</p>}
      </div>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </div>
  );
}

export function Panel({ title, actions, children, className, bodyClass }: { title?: ReactNode; actions?: ReactNode; children: ReactNode; className?: string; bodyClass?: string }) {
  return (
    <section className={cn("rounded-xl border border-line bg-panel", className)}>
      {(title || actions) && (
        <div className="flex h-11 items-center justify-between gap-3 border-b border-line px-4">
          <h2 className="text-[13px] font-semibold">{title}</h2>
          {actions && <div className="flex items-center gap-1.5">{actions}</div>}
        </div>
      )}
      <div className={cn("p-4", bodyClass)}>{children}</div>
    </section>
  );
}

export function EmptyState({ icon, title, text, action, compact }: { icon: ReactNode; title: string; text?: ReactNode; action?: ReactNode; compact?: boolean }) {
  return (
    <div className={cn("flex flex-col items-center justify-center text-center", compact ? "py-6" : "py-14")}>
      <div className="mb-3 flex h-11 w-11 items-center justify-center rounded-full bg-raised text-faint">{icon}</div>
      <div className="text-[14px] font-medium">{title}</div>
      {text && <div className="mt-1 max-w-sm text-[12px] text-muted">{text}</div>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

export function StatusIcon({ level, className }: { level: CheckLevel; className?: string }) {
  const c = cn("h-4 w-4 shrink-0", className);
  if (level === "ok") return <CheckCircle2 className={cn(c, "text-success")} />;
  if (level === "warn") return <CircleAlert className={cn(c, "text-amber-500")} />;
  if (level === "bad") return <XCircle className={cn(c, "text-danger")} />;
  return <Spinner className={cn("h-3.5 w-3.5 text-faint", className)} />;
}

/** "Coming in phase N" note on pages whose engine is not built yet. */
export function PhaseNotice({ phase, title, children }: { phase: number; title: string; children: ReactNode }) {
  return (
    <div className="mb-5 flex gap-3 rounded-xl border border-accent/30 bg-accent/[0.07] px-4 py-3">
      <Info className="mt-0.5 h-4 w-4 shrink-0 text-accent" />
      <div className="min-w-0 text-[13px]">
        <div className="font-medium">
          {title} <span className="ml-1 rounded bg-accent/20 px-1.5 py-px text-[11px] font-semibold text-accent">Phase {phase}</span>
        </div>
        <div className="mt-0.5 text-muted">{children}</div>
      </div>
    </div>
  );
}

export function Row({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children?: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-line/60 py-2.5 last:border-0">
      <div className="min-w-0">
        <div className="text-[13px]">{label}</div>
        {hint && <div className="mt-0.5 truncate text-[11px] text-faint">{hint}</div>}
      </div>
      <div className="shrink-0 text-right text-[13px] text-muted">{children}</div>
    </div>
  );
}

export function Pending({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-2 py-6 text-[13px] text-muted">
      <CircleDashed className="h-4 w-4 animate-spin" />
      {label}
    </div>
  );
}
