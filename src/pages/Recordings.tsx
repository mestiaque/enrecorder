import { CheckSquare, Clapperboard, FolderOpen, LayoutGrid, List, Search, SlidersHorizontal, Tag as TagIcon, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { MediaRow, MediaTile } from "../components/media/MediaList";
import { Popover } from "../components/ui/menus";
import { confirmDialog, promptDialog } from "../store/ui";
import type { Tag } from "../lib/types";
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

const VIEW_KEY = "enrecorder.libraryView";
const loadView = (): "list" | "grid" => {
  try {
    return localStorage.getItem(VIEW_KEY) === "grid" ? "grid" : "list";
  } catch {
    return "list";
  }
};

type More = Pick<LibraryQuery, "date" | "length" | "size" | "resolution" | "tag">;
const NO_MORE: More = { date: "", length: "", size: "", resolution: "", tag: "" };

/** Date, length, size, resolution and tag filters, plus tag management. */
function MoreFilters({ value, onChange, tags, reloadTags }: { value: More; onChange: (m: More) => void; tags: Tag[]; reloadTags: () => void }) {
  const row = (label: string, el: React.ReactNode) => (
    <div className="flex items-center justify-between gap-4 py-1.5">
      <span className="text-[12px] text-muted">{label}</span>
      {el}
    </div>
  );
  return (
    <div className="w-[300px]">
      {row("Date", <Select className="w-40" value={value.date ?? ""} onChange={(v) => onChange({ ...value, date: v })} options={[{ value: "", label: "Any time" }, { value: "today", label: "Today" }, { value: "week", label: "Last 7 days" }, { value: "month", label: "Last 30 days" }]} />)}
      {row("Length", <Select className="w-40" value={value.length ?? ""} onChange={(v) => onChange({ ...value, length: v })} options={[{ value: "", label: "Any" }, { value: "short", label: "Under 1 min" }, { value: "medium", label: "1–10 min" }, { value: "long", label: "Over 10 min" }]} />)}
      {row("File size", <Select className="w-40" value={value.size ?? ""} onChange={(v) => onChange({ ...value, size: v })} options={[{ value: "", label: "Any" }, { value: "small", label: "Under 50 MB" }, { value: "medium", label: "50–500 MB" }, { value: "large", label: "Over 500 MB" }]} />)}
      {row("Resolution", <Select className="w-40" value={value.resolution ?? ""} onChange={(v) => onChange({ ...value, resolution: v })} options={[{ value: "", label: "Any" }, { value: "uhd", label: "4K" }, { value: "fhd", label: "1080p or more" }, { value: "hd", label: "720p or more" }, { value: "sd", label: "Below 720p" }]} />)}
      {row("Tag", <Select className="w-40" value={value.tag ?? ""} onChange={(v) => onChange({ ...value, tag: v })} options={[{ value: "", label: "Any" }, ...tags.map((t) => ({ value: t.id, label: `${t.name} (${t.count})` }))]} />)}
      <div className="mt-2 border-t border-line pt-2">
        <div className="mb-1 text-[11px] font-semibold uppercase tracking-wider text-faint">Tags</div>
        <div className="flex flex-wrap gap-1">
          {tags.map((t) => (
            <span key={t.id} className="flex items-center gap-1 rounded bg-accent/15 px-1.5 py-0.5 text-[11px] text-accent">
              {t.name}
              <button
                aria-label={`Delete tag ${t.name}`}
                className="rounded hover:text-danger"
                onClick={async () => {
                  if (!(await confirmDialog({ title: `Delete the tag "${t.name}"?`, message: "It is removed from every item. The files are not touched.", confirmLabel: "Delete tag", danger: true }))) return;
                  await api.deleteTag(t.id).catch((e) => toast({ kind: "error", title: "Could not delete the tag", detail: errorMessage(e) }));
                  if (value.tag === t.id) onChange({ ...value, tag: "" });
                  reloadTags();
                }}
              >
                <X className="h-3 w-3" />
              </button>
            </span>
          ))}
          {!tags.length && <span className="text-[11px] text-faint">No tags yet — add them to items in their details.</span>}
        </div>
      </div>
      <div className="mt-2 flex justify-end">
        <Button size="sm" variant="ghost" onClick={() => onChange(NO_MORE)}>
          Clear filters
        </Button>
      </div>
    </div>
  );
}

export function RecordingsPage() {
  const [filter, setFilter] = useState<LibraryQuery["filter"]>("all");
  const [more, setMore] = useState<More>(NO_MORE);
  const [view, setViewState] = useState<"list" | "grid">(loadView);
  const setView = (v: "list" | "grid") => {
    setViewState(v);
    try {
      localStorage.setItem(VIEW_KEY, v);
    } catch {
      /* only for this session */
    }
  };
  const [selecting, setSelecting] = useState(false);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [tags, setTags] = useState<Tag[]>([]);
  const reloadTags = () => void api.listTags().then(setTags).catch(() => {});
  const [sort, setSort] = useState<LibraryQuery["sort"]>("newest");
  const [typed, setTyped] = useState("");
  const [search, setSearch] = useState("");
  // Search after typing pauses, not on every key.
  useEffect(() => {
    const t = window.setTimeout(() => setSearch(typed.trim()), 250);
    return () => window.clearTimeout(t);
  }, [typed]);
  const media = useMedia({ filter, sort, search, ...more });
  const qc = useQueryClient();
  // Files moved or deleted outside ENRecorder show up as "missing" (checked on open).
  useEffect(() => {
    reloadTags();
    void api
      .checkLibraryFiles()
      .then(([gone, back]) => {
        if (gone || back) void qc.invalidateQueries({ queryKey: ["media"] });
        if (gone) toast({ kind: "info", title: `${gone} file${gone > 1 ? "s were" : " was"} moved or deleted outside ENRecorder`, detail: "They are marked “missing” in the list." });
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const toggle = (id: string) =>
    setChecked((c) => {
      const n = new Set(c);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const bulkDelete = async () => {
    const ids = [...checked];
    if (!ids.length) return;
    if (!(await confirmDialog({ title: `Move ${ids.length} item${ids.length > 1 ? "s" : ""} to the trash?`, message: "The files go to the system trash (you can restore them there) and leave the library.", confirmLabel: "Move to trash", danger: true }))) return;
    try {
      const n = await api.deleteMediaMany(ids);
      toast({ kind: "success", title: `${n} item${n > 1 ? "s" : ""} moved to the trash` });
    } catch (e) {
      toast({ kind: "error", title: "Some items could not be deleted", detail: errorMessage(e) });
    }
    setChecked(new Set());
    setSelected(null);
    void qc.invalidateQueries({ queryKey: ["media"] });
    void qc.invalidateQueries({ queryKey: ["dashboard"] });
  };
  const bulkTag = async () => {
    const ids = [...checked];
    const name = await promptDialog({ title: `Tag ${ids.length} item${ids.length > 1 ? "s" : ""}`, label: "Tag name (new or existing)", initial: "", confirmLabel: "Add tag" });
    if (!name?.trim()) return;
    try {
      const tag = await api.createTag(name.trim());
      for (const id of ids) {
        const current = await api.mediaTagIds(id);
        if (!current.includes(tag.id)) await api.setMediaTags([id], [...current, tag.id]);
      }
      toast({ kind: "success", title: `Tagged “${tag.name}”` });
      reloadTags();
      void qc.invalidateQueries({ queryKey: ["media"] });
    } catch (e) {
      toast({ kind: "error", title: "Could not tag", detail: errorMessage(e) });
    }
  };
  const moreCount = Object.values(more).filter(Boolean).length;
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState<Screenshot | null>(null);
  const editShot = (id: string) => void api.getScreenshot(id).then(setEditing).catch((e) => toast({ kind: "error", title: "Could not open", detail: errorMessage(e) }));
  const { data: paths } = usePaths();
  const items = media.data ?? [];
  const filtered = filter !== "all" || search !== "" || Object.values(more).some(Boolean);

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
        <Popover
          trigger={
            <Button title="More filters">
              <SlidersHorizontal className="h-3.5 w-3.5" />
              Filters{moreCount ? ` (${moreCount})` : ""}
            </Button>
          }
        >
          <MoreFilters value={more} onChange={setMore} tags={tags} reloadTags={reloadTags} />
        </Popover>
        <Select value={sort} onChange={setSort} options={SORTS} />
        <Segmented value={view} onChange={setView} options={[{ value: "list", label: <List className="h-3.5 w-3.5" />, title: "List" }, { value: "grid", label: <LayoutGrid className="h-3.5 w-3.5" />, title: "Grid" }]} />
        <Button
          variant={selecting ? "primary" : "default"}
          onClick={() => {
            setSelecting(!selecting);
            setChecked(new Set());
          }}
          title="Select several items"
        >
          <CheckSquare className="h-3.5 w-3.5" />
          Select
        </Button>
      </div>
      {selecting && (
        <div className="mb-3 flex flex-wrap items-center gap-2 rounded-lg border border-line bg-panel px-3 py-2 text-[12px]">
          <span className="text-muted">{checked.size} selected</span>
          <Button size="sm" variant="ghost" onClick={() => setChecked(new Set(items.map((m) => m.id)))}>
            Select all
          </Button>
          <Button size="sm" variant="ghost" disabled={!checked.size} onClick={() => setChecked(new Set())}>
            Clear
          </Button>
          <span className="flex-1" />
          <Button size="sm" disabled={!checked.size} onClick={() => void bulkTag()}>
            <TagIcon className="h-3.5 w-3.5" />
            Tag…
          </Button>
          <Button size="sm" variant="danger" disabled={!checked.size} onClick={() => void bulkDelete()}>
            <Trash2 className="h-3.5 w-3.5" />
            Move to trash
          </Button>
        </div>
      )}
      <div className="flex items-start gap-4">
      <Panel bodyClass="p-2" className="min-w-0 flex-1">
        {media.isError ? (
          <EmptyState compact icon={<Clapperboard className="h-5 w-5" />} title="Could not load the library" text={errorMessage(media.error)} action={<Button onClick={() => void media.refetch()}>Try again</Button>} />
        ) : items.length ? (
          view === "grid" ? (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3 p-1">
              {items.map((m) => (
                <MediaTile key={`${m.kind}-${m.id}`} item={m} selected={selected === m.id} onSelect={() => (selecting ? toggle(m.id) : setSelected(m.id))} thumb={<Thumb item={m} className="h-full w-full" />} checked={checked.has(m.id)} onCheck={selecting ? () => toggle(m.id) : undefined} />
              ))}
            </div>
          ) : (
            items.map((m) => <MediaRow key={`${m.kind}-${m.id}`} item={m} selected={selected === m.id} onSelect={() => (selecting ? toggle(m.id) : setSelected(m.id))} thumb={<Thumb item={m} className="h-10 w-16" />} checked={checked.has(m.id)} onCheck={selecting ? () => toggle(m.id) : undefined} />)
          )
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
