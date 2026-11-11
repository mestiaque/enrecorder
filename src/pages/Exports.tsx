import { ListVideo } from "lucide-react";
import { EmptyState, PageHeader, Panel } from "../components/ui/blocks";
import { basename, formatDate } from "../lib/format";
import { useExportJobs } from "../lib/queries";

const STATUS: Record<string, string> = {
  queued: "Queued",
  initializing: "Starting",
  running: "Running",
  finalizing: "Finishing",
  completed: "Done",
  failed: "Failed",
  cancelled: "Cancelled",
};

export function ExportsPage() {
  const jobs = useExportJobs();
  const list = jobs.data ?? [];
  return (
    <>
      <PageHeader title="Export Center" subtitle="Conversions and exports run here, one queue for the whole app." />
      <Panel bodyClass="p-2">
        {list.length ? (
          list.map((j) => (
            <div key={j.id} className="flex items-center gap-3 rounded-lg px-2 py-2 hover:bg-raised">
              <ListVideo className="h-4 w-4 shrink-0 text-accent" />
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px]">{basename(j.outputPath)}</div>
                <div className="truncate text-[11px] text-faint">{j.error ?? `from ${basename(j.sourcePath)}`}</div>
              </div>
              <span className={j.status === "failed" ? "text-[12px] text-danger" : "text-[12px] text-muted"}>{STATUS[j.status] ?? j.status}</span>
              <span className="w-36 shrink-0 text-right text-[11px] text-faint">{formatDate(j.finishedAt ?? j.createdAt)}</span>
            </div>
          ))
        ) : (
          <EmptyState
            icon={<ListVideo className="h-5 w-5" />}
            title={jobs.isLoading ? "Loading…" : "No exports yet"}
            text="When you export or convert a recording (phase 5), each job shows its progress here and can be cancelled or retried."
          />
        )}
      </Panel>
    </>
  );
}
