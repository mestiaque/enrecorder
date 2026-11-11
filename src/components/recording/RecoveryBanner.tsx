import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LifeBuoy } from "lucide-react";
import { useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { formatBytes, formatDate } from "../../lib/format";
import { confirmDialog, toast } from "../../store/ui";
import { Button } from "../ui/primitives";

/** Recordings a crash or power cut interrupted: save what was recorded, or throw it away. */
export function RecoveryBanner() {
  const qc = useQueryClient();
  const list = useQuery({ queryKey: ["interrupted"], queryFn: api.listInterrupted, staleTime: 60_000 });
  const [busy, setBusy] = useState<string | null>(null);
  if (!list.data?.length) return null;
  const done = () => {
    void qc.invalidateQueries({ queryKey: ["interrupted"] });
    void qc.invalidateQueries({ queryKey: ["media"] });
    void qc.invalidateQueries({ queryKey: ["dashboard"] });
  };
  const recover = async (id: string) => {
    setBusy(id);
    try {
      const path = await api.recoverRecording(id);
      toast({ kind: "success", title: "Recording recovered", detail: path });
    } catch (e) {
      toast({ kind: "error", title: "Could not recover", detail: errorMessage(e) }, 10000);
    } finally {
      setBusy(null);
      done();
    }
  };
  const discard = async (id: string) => {
    if (!(await confirmDialog({ title: "Throw away this recording?", message: "The parts recorded before the interruption are deleted.", confirmLabel: "Delete", danger: true }))) return;
    await api.discardRecording(id).catch((e) => toast({ kind: "error", title: "Could not delete", detail: errorMessage(e) }));
    done();
  };
  return (
    <section className="mb-5 rounded-xl border border-amber-500/40 bg-amber-500/10 p-4">
      <div className="flex items-center gap-2 text-[13px] font-medium">
        <LifeBuoy className="h-4 w-4 text-amber-500" />
        {list.data.length === 1 ? "A recording was interrupted" : `${list.data.length} recordings were interrupted`}
      </div>
      <p className="mt-1 text-[12px] text-muted">ENRecorder or the computer stopped while recording. What was recorded until then can usually be saved.</p>
      <div className="mt-3 space-y-2">
        {list.data.map((i) => (
          <div key={i.sessionId} className="flex flex-wrap items-center gap-3 rounded-lg bg-panel px-3 py-2 text-[12px]">
            <span className="min-w-0 flex-1">
              {i.kind === "audio" ? "Audio" : "Screen recording"} from {formatDate(i.startedAt)} · {i.parts} part{i.parts > 1 ? "s" : ""} · {formatBytes(i.sizeBytes)}
            </span>
            <Button size="sm" variant="primary" disabled={!!busy} onClick={() => void recover(i.sessionId)}>
              {busy === i.sessionId ? "Recovering…" : "Recover"}
            </Button>
            <Button size="sm" variant="ghost" disabled={!!busy} onClick={() => void discard(i.sessionId)}>
              Discard
            </Button>
          </div>
        ))}
      </div>
    </section>
  );
}
