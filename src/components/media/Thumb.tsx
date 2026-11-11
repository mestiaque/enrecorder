import { convertFileSrc } from "@tauri-apps/api/core";
import { useThumbnail } from "../../lib/queries";
import { cn } from "../../lib/format";
import type { MediaItem } from "../../lib/types";
import { KindIcon } from "./MediaList";

/** Library thumbnail (video frame, audio waveform, scaled screenshot) with an icon fallback. */
export function Thumb({ item, className }: { item: MediaItem; className?: string }) {
  const t = useThumbnail(item.id);
  return (
    <span className={cn("relative flex shrink-0 items-center justify-center overflow-hidden rounded-md bg-raised text-accent", className)}>
      {t.data ? <img src={convertFileSrc(t.data)} alt="" className="h-full w-full object-cover" draggable={false} /> : <KindIcon kind={item.kind} />}
    </span>
  );
}
