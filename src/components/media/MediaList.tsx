/** Rows and tiles for library items (recordings and screenshots). */
import { FileAudio, FileVideo, Image as ImageIcon, Star } from "lucide-react";
import { api, errorMessage } from "../../lib/api";
import { cn, formatBytes, formatDate, formatDuration } from "../../lib/format";
import type { MediaItem } from "../../lib/types";
import { toast } from "../../store/ui";

export function KindIcon({ kind, className }: { kind: MediaItem["kind"]; className?: string }) {
  const c = cn("h-4 w-4", className);
  if (kind === "video") return <FileVideo className={c} />;
  if (kind === "audio") return <FileAudio className={c} />;
  return <ImageIcon className={c} />;
}

function details(m: MediaItem): string {
  const parts: string[] = [];
  if (m.durationMs != null) parts.push(formatDuration(m.durationMs));
  if (m.width && m.height) parts.push(`${m.width}×${m.height}`);
  parts.push(formatBytes(m.sizeBytes));
  return parts.join(" · ");
}

const open = (m: MediaItem) => api.openPath(m.path).catch((e) => toast({ kind: "error", title: "Could not open the file", detail: errorMessage(e) }));

export function MediaRow({ item, selected, onSelect, thumb }: { item: MediaItem; selected?: boolean; onSelect?: () => void; thumb?: React.ReactNode }) {
  return (
    <button onClick={onSelect} onDoubleClick={() => void open(item)} className={cn("flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left hover:bg-raised", selected && "bg-accent/10 hover:bg-accent/15")} title="Double-click to open">
      {thumb ?? (
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-raised text-accent">
          <KindIcon kind={item.kind} />
        </span>
      )}
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-1.5 truncate text-[13px]">
          {item.title}
          {item.favorite && <Star className="h-3 w-3 fill-amber-400 text-amber-400" />}
          {item.status !== "ready" && <span className="rounded bg-amber-500/15 px-1 text-[10px] text-amber-500">{item.status}</span>}
        </span>
        <span className="block truncate text-[11px] text-faint">{details(item)}</span>
      </span>
      <span className="shrink-0 text-[11px] text-faint">{formatDate(item.createdAt)}</span>
    </button>
  );
}
