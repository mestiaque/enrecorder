import { CircleSlash, FolderOpen, ListVideo, RotateCcw } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { EmptyState, PageHeader, Panel } from "../components/ui/blocks";
import { Button, ProgressBar } from "../components/ui/primitives";
import { api, errorMessage } from "../lib/api";
import { basename, cn, formatDate } from "../lib/format";
import { useExportJobs } from "../lib/queries";
import { toast } from "../store/ui";

const STATUS: Record<string, string> = {
  queued: "Waiting",
  initializing: "Starting",
  running: "Running",
  finalizing: "Finishing",
  completed: "Done",
  failed: "Failed",
  cancelled: "Cancelled",
};

export function ExportsPage() {
  const qc = useQueryClient();
  const jobs = useExportJobs();
  const list = jobs.data ?? [];
  const active = list.filter((j) => ["queued", "initializing", "running", "finalizing"].includes(j.status)).length;
  return (
    <>
      <PageHeader title="Export Center" subtitle={active ? `${active} export${active > 1 ? "s" : ""} in the queue — one runs at a time.` : "Conversions and exports run here, one queue for the whole app."} />
      <Panel bodyClass="p-2">
        {list.length ? (
          list.map((j) => {
            const live = ["queued", "initializing", "running", "finalizing"].includes(j.status);
            return (
              <div key={j.id} className="flex items-center gap-3 rounded-lg px-2 py-2 hover:bg-raised">
                <ListVideo className="h-4 w-4 shrink-0 text-accent" />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px]">{basename(j.outputPath)}</div>
                  {j.status === "running" ? (
                    <div className="mt-1 flex items-center gap-2">
                      <ProgressBar value={j.progress} total={1} className="max-w-sm" />
                      <span className="text-[11px] tabular-nums text-muted">{Math.round(j.progress * 100)}%</span>
                    </div>
                  ) : (
                    <div className={cn("truncate text-[11px]", j.status === "failed" ? "text-danger" : "text-faint")}>{j.error ?? `from ${basename(j.sourcePath)}`}</div>
                  )}
                </div>
                <span className={j.status === "failed" ? "text-[12px] text-danger" : "text-[12px] text-muted"}>{STATUS[j.status] ?? j.status}</span>
                {live ? (
                  <Button size="sm" variant="ghost" onClick={() => void api.cancelExport(j.id).catch((e) => toast({ kind: "error", title: "Could not cancel", detail: errorMessage(e) }))} title="Cancel">
                    <CircleSlash className="h-3.5 w-3.5" />
                  </Button>
                ) : j.status === "completed" ? (
                  <Button size="sm" variant="ghost" onClick={() => void api.revealPath(j.outputPath).catch((e) => toast({ kind: "error", title: "Could not show the file", detail: errorMessage(e) }))} title="Show in folder">
                    <FolderOpen className="h-3.5 w-3.5" />
                  </Button>
                ) : j.status === "failed" || j.status === "cancelled" ? (
                  <Button
                    size="sm"
                    variant="ghost"
                    title="Run this export again"
                    onClick={() =>
                      void api
                        .retryExport(j.id)
                        .then(() => void qc.invalidateQueries({ queryKey: ["exports"] }))
                        .catch((e) => toast({ kind: "error", title: "Could not retry", detail: errorMessage(e) }))
                    }
                  >
                    <RotateCcw className="h-3.5 w-3.5" />
                  </Button>
                ) : (
                  <span className="w-8" />
                )}
                <span className="w-36 shrink-0 text-right text-[11px] text-faint">{formatDate(j.finishedAt ?? j.createdAt)}</span>
              </div>
            );
          })
        ) : (
          <EmptyState icon={<ListVideo className="h-5 w-5" />} title={jobs.isLoading ? "Loading…" : "No exports yet"} text="Export or trim a recording (Recordings → a recording → Export… or Trim) and its progress shows here." />
        )}
      </Panel>
    </>
  );
}
