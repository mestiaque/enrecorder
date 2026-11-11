import { Clapperboard, FolderOpen, Search, X } from "lucide-react";
import { useEffect, useState } from "react";
import { MediaRow } from "../components/media/MediaList";
import { MediaDetailPanel } from "../components/media/MediaDetail";
import { Thumb } from "../components/media/Thumb";
import { ScreenshotEditor } from "../components/screenshot/Editor";
import type { Screenshot } from "../lib/types";
import { useQueryClient } from "@tanstack/react-query";
import { EmptyState, PageHeader, Panel } from "../components/ui/blocks";
import { Button, Segmented, Select } from "../components/ui/primitives";
import { api, errorMessage } from "../lib/api";
import { usePaths, useMedia } from "../lib/queries";
import type { LibraryQuery } from "../lib/types";
import { toast } from "../store/ui";

const FILTERS: { value: LibraryQuery["filter"]; label: string }[] = [
  { value: "all", label: "All" },
  { value: "video", label: "Video" },
  { value: "audio", label: "Audio" },
  { value: "screenshot", label: "Screenshots" },
  { value: "favorites", label: "Favorites" },
];

const SORTS: { value: LibraryQuery["sort"]; label: string }[] = [
  { value: "newest", label: "Newest first" },
  { value: "oldest", label: "Oldest first" },
  { value: "name", label: "Name" },
  { value: "size", label: "Largest" },
  { value: "duration", label: "Longest" },
];

export function RecordingsPage() {
  const [filter, setFilter] = useState<LibraryQuery["filter"]>("all");
  const [sort, setSort] = useState<LibraryQuery["sort"]>("newest");
  const [typed, setTyped] = useState("");
  const [search, setSearch] = useState("");
  // Search after typing pauses, not on every key.
  useEffect(() => {
    const t = window.setTimeout(() => setSearch(typed.trim()), 250);
    return () => window.clearTimeout(t);
  }, [typed]);
  const media = useMedia({ filter, sort, search });
  const qc = useQueryClient();
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState<Screenshot | null>(null);
  const editShot = (id: string) => void api.getScreenshot(id).then(setEditing).catch((e) => toast({ kind: "error", title: "Could not open", detail: errorMessage(e) }));
  const { data: paths } = usePaths();
  const items = media.data ?? [];
  const filtered = filter !== "all" || search !== "";

  return (
    <>
      <PageHeader
        title="Recordings"
        subtitle="Everything you record or capture, in one place."
        actions={
          <Button
            disabled={!paths}
            onClick={() => paths && api.openFolder(paths.recordings).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) }))}
          >
            <FolderOpen className="h-3.5 w-3.5" />
            Open folder
          </Button>
        }
      />
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <Segmented value={filter} onChange={setFilter} options={FILTERS} />
        <div className="relative ml-auto w-64">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-faint" />
          <input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setTyped("")}
            placeholder="Search by name"
            aria-label="Search recordings"
            className="h-8 w-full rounded-md border border-line bg-panel pl-8 pr-7 text-[13px] outline-none placeholder:text-faint focus:border-accent"
          />
          {typed && (
            <button className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded p-0.5 text-faint hover:text-fg" onClick={() => setTyped("")} aria-label="Clear search">
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
        <Select value={sort} onChange={setSort} options={SORTS} />
      </div>
      <div className="flex items-start gap-4">
      <Panel bodyClass="p-2" className="min-w-0 flex-1">
        {media.isError ? (
          <EmptyState compact icon={<Clapperboard className="h-5 w-5" />} title="Could not load the library" text={errorMessage(media.error)} action={<Button onClick={() => void media.refetch()}>Try again</Button>} />
        ) : items.length ? (
          items.map((m) => <MediaRow key={`${m.kind}-${m.id}`} item={m} selected={selected === m.id} onSelect={() => setSelected(m.id)} thumb={<Thumb item={m} className="h-10 w-16" />} />)
        ) : (
          <EmptyState
            icon={<Clapperboard className="h-5 w-5" />}
            title={media.isLoading ? "Loading…" : filtered ? "Nothing matches" : "No recordings yet"}
            text={filtered ? "Try another filter or search." : "Recordings, audio and screenshots you make with ENRecorder appear here, with search, tags and favorites."}
          />
        )}
      </Panel>
      {selected && <MediaDetailPanel id={selected} onClose={() => setSelected(null)} onEditScreenshot={editShot} />}
      </div>
      {items.length > 0 && <div className="mt-2 text-[11px] text-faint">{items.length.toLocaleString()} item{items.length > 1 ? "s" : ""} · click for details, double-click to open</div>}
      {editing && (
        <ScreenshotEditor
          shot={editing}
          startTool="select"
          onClose={() => {
            setEditing(null);
            void qc.invalidateQueries({ queryKey: ["media"] });
            void qc.invalidateQueries({ queryKey: ["thumb"] });
          }}
        />
      )}
    </>
  );
}
