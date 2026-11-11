import { useQuery, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Camera, Captions, ExternalLink, FolderOpen, Loader2, Pencil, Plus, RefreshCw, Scissors, Share2, Star, Trash2, Type, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Tag } from "../../lib/types";
import { api, errorMessage } from "../../lib/api";
import { useMediaUrl } from "../../lib/queries";
import { formatBytes, formatDate, formatDuration } from "../../lib/format";
import { useEditorTarget } from "../../store/editor";
import { useNav } from "../../store/nav";
import { useTranscribe } from "../../store/transcribe";
import { confirmDialog, promptDialog, toast } from "../../store/ui";
import { Button, IconButton } from "../ui/primitives";
import { ExportDialog } from "./ExportDialog";

const fail = (what: string) => (e: unknown) => toast({ kind: "error", title: `Could not ${what}`, detail: errorMessage(e) });

/** Side panel: preview (video, audio or image), facts, and the library actions. */
export function MediaDetailPanel({ id, onClose, onEditScreenshot }: { id: string; onClose: () => void; onEditScreenshot: (id: string) => void }) {
  const qc = useQueryClient();
  const d = useQuery({ queryKey: ["media-item", id], queryFn: () => api.getMedia(id) });
  const stream = useMediaUrl(id);
  const [exporting, setExporting] = useState(false);
  const player = useRef<HTMLVideoElement>(null);
  const [allTags, setAllTags] = useState<Tag[]>([]);
  const [mine, setMine] = useState<string[]>([]);
  const [newTag, setNewTag] = useState("");
  useEffect(() => {
    void api.listTags().then(setAllTags).catch(() => {});
    void api.mediaTagIds(id).then(setMine).catch(() => {});
  }, [id]);
  const saveTags = async (ids: string[]) => {
    setMine(ids);
    await api.setMediaTags([id], ids).catch(fail("save the tags"));
    void api.listTags().then(setAllTags).catch(() => {});
    void qc.invalidateQueries({ queryKey: ["media"] });
  };
  const addTag = async () => {
    const name = newTag.trim();
    if (!name) return;
    try {
      const t = await api.createTag(name);
      setNewTag("");
      if (!mine.includes(t.id)) await saveTags([...mine, t.id]);
    } catch (e) {
      fail("add the tag")(e);
    }
  };
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["media"] });
    void qc.invalidateQueries({ queryKey: ["media-item", id] });
    void qc.invalidateQueries({ queryKey: ["dashboard"] });
  };
  if (!d.data) return <aside className="w-[380px] shrink-0 rounded-xl border border-line bg-panel p-4 text-[13px] text-muted">{d.isError ? errorMessage(d.error) : "Loading…"}</aside>;
  const { item, metadata: m, exists } = d.data;
  const src = convertFileSrc(item.path);
  // Video/audio stream from the preview server (WebKitGTK cannot play asset: URLs).
  const play = stream.data ? `${stream.data}?v=${item.sizeBytes}` : "";

  const rename = async () => {
    const title = await promptDialog({ title: "Rename", label: "Name", initial: item.title, confirmLabel: "Rename" });
    if (title && title !== item.title) await api.renameMedia(item.id, title).then(refresh).catch(fail("rename"));
  };
  const remove = async () => {
    if (!(await confirmDialog({ title: `Move “${item.title}” to the trash?`, message: "The file goes to the system trash (you can restore it from there) and leaves the library.", confirmLabel: "Move to trash", danger: true }))) return;
    await api
      .deleteMedia(item.id)
      .then(() => {
        onClose();
        refresh();
      })
      .catch(fail("delete"));
  };
  const facts: [string, string][] = [
    ["Created", formatDate(item.createdAt)],
    ["Size", formatBytes(item.sizeBytes)],
    ...(item.durationMs != null ? [["Length", formatDuration(item.durationMs)] as [string, string]] : []),
    ...(item.width ? [["Dimensions", `${item.width}×${item.height}`] as [string, string]] : []),
    ...(m?.videoCodec ? [["Video", `${m.videoCodec}${m.fps ? ` · ${m.fps.toFixed(0)} fps` : ""}`] as [string, string]] : []),
    ...(m?.audioCodec ? [["Audio", `${m.audioCodec}${m.audioStreams > 1 ? ` · ${m.audioStreams} tracks` : ""}`] as [string, string]] : []),
    ...(m?.bitrate ? [["Bitrate", `${(m.bitrate / 1_000_000).toFixed(1)} Mbit/s`] as [string, string]] : []),
  ];

  return (
    <aside className="flex w-[380px] shrink-0 flex-col overflow-hidden rounded-xl border border-line bg-panel">
      <div className="flex items-center gap-2 border-b border-line px-3 py-2">
        <span className="min-w-0 flex-1 truncate text-[13px] font-medium" title={item.title}>{item.title}</span>
        <IconButton label={item.favorite ? "Remove from favorites" : "Add to favorites"} active={item.favorite} onClick={() => void api.setFavorite(item.id, !item.favorite).then(refresh).catch(fail("change the favorite"))}>
          <Star className={item.favorite ? "h-4 w-4 fill-amber-400 text-amber-400" : "h-4 w-4"} />
        </IconButton>
        <IconButton label="Close" onClick={onClose}>
          <X className="h-4 w-4" />
        </IconButton>
      </div>
      <div className="bg-black">
        {!exists ? (
          <div className="p-6 text-center text-[12px] text-danger">The file is missing (moved or deleted outside ENRecorder).</div>
        ) : item.kind === "video" ? (
          <video ref={player} key={play} src={play} controls className="aspect-video w-full" preload="metadata" />
        ) : item.kind === "audio" ? (
          <audio key={play} src={play} controls className="w-full p-3" preload="metadata" />
        ) : (
          <img src={`${src}?v=${item.sizeBytes}`} alt="" className="max-h-64 w-full object-contain" />
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        <dl className="divide-y divide-line rounded-lg bg-raised px-3 text-[12px]">
          {facts.map(([k, v]) => (
            <div key={k} className="flex justify-between gap-3 py-2">
              <dt className="text-faint">{k}</dt>
              <dd className="truncate text-right">{v}</dd>
            </div>
          ))}
        </dl>
        <div className="mt-1 truncate px-1 font-mono text-[10px] text-faint" title={item.path}>{item.path}</div>
        <div className="mt-3">
          <div className="mb-1 text-[11px] font-semibold uppercase tracking-wider text-faint">Tags</div>
          <div className="flex flex-wrap items-center gap-1">
            {allTags
              .filter((t) => mine.includes(t.id))
              .map((t) => (
                <span key={t.id} className="flex items-center gap-1 rounded bg-accent/15 px-1.5 py-0.5 text-[11px] text-accent">
                  {t.name}
                  <button aria-label={`Remove tag ${t.name}`} onClick={() => void saveTags(mine.filter((x) => x !== t.id))}>
                    <X className="h-3 w-3" />
                  </button>
                </span>
              ))}
            {allTags
              .filter((t) => !mine.includes(t.id))
              .slice(0, 6)
              .map((t) => (
                <button key={t.id} className="rounded border border-dashed border-line px-1.5 py-0.5 text-[11px] text-muted hover:border-accent hover:text-accent" onClick={() => void saveTags([...mine, t.id])}>
                  + {t.name}
                </button>
              ))}
          </div>
          <div className="mt-1.5 flex gap-1.5">
            <input
              value={newTag}
              onChange={(e) => setNewTag(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void addTag()}
              placeholder="New tag"
              aria-label="New tag"
              maxLength={40}
              className="h-7 min-w-0 flex-1 rounded-md border border-line bg-bg px-2 text-[12px] outline-none placeholder:text-faint focus:border-accent"
            />
            <Button size="sm" disabled={!newTag.trim()} onClick={() => void addTag()}>
              <Plus className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
        <div className="mt-3 grid grid-cols-2 gap-2">
          {item.kind === "screenshot" ? (
            <Button onClick={() => onEditScreenshot(item.id)}>
              <Pencil className="h-3.5 w-3.5" />
              Annotate
            </Button>
          ) : (
            <Button
              onClick={() => {
                useEditorTarget.getState().open(item.id);
                useNav.getState().go("editor");
              }}
            >
              <Scissors className="h-3.5 w-3.5" />
              Trim
            </Button>
          )}
          {item.kind !== "screenshot" && (
            <Button onClick={() => setExporting(true)}>
              <Share2 className="h-3.5 w-3.5" />
              Export…
            </Button>
          )}
          <Button onClick={() => void rename()}>
            <Type className="h-3.5 w-3.5" />
            Rename
          </Button>
          <Button onClick={() => void api.revealPath(item.path).catch(fail("show the file"))}>
            <FolderOpen className="h-3.5 w-3.5" />
            Show in folder
          </Button>
          <Button onClick={() => void api.openPath(item.path).catch(fail("open the file"))}>
            <ExternalLink className="h-3.5 w-3.5" />
            Open with…
          </Button>
          {item.kind === "video" && exists && (
            <Button
              title="Save the frame shown in the player as a screenshot"
              onClick={() =>
                void api
                  .extractFrame(item.id, Math.round((player.current?.currentTime ?? 0) * 1000))
                  .then(() => {
                    toast({ kind: "success", title: "Frame saved", detail: "It is in the library with your screenshots." });
                    refresh();
                  })
                  .catch(fail("save the frame"))
              }
            >
              <Camera className="h-3.5 w-3.5" />
              Save frame
            </Button>
          )}
          {item.kind !== "screenshot" && exists && <TranscribeButton id={item.id} title={item.title} />}
          <Button
            title="Read the file again (size, length, codecs) and make a new thumbnail"
            onClick={() =>
              void api
                .reprocessMedia(item.id)
                .then(() => {
                  toast({ kind: "success", title: "Updated from the file" });
                  refresh();
                  void qc.invalidateQueries({ queryKey: ["thumb"] });
                })
                .catch(fail("read the file"))
            }
          >
            <RefreshCw className="h-3.5 w-3.5" />
            Re-read file
          </Button>
          <Button variant="ghost" className="text-danger hover:text-danger" onClick={() => void remove()}>
            <Trash2 className="h-3.5 w-3.5" />
            Delete
          </Button>
        </div>
      </div>
      {exporting && <ExportDialog item={item} open onClose={() => setExporting(false)} />}
    </aside>
  );
}

/** Subtitles from speech (whisper.cpp, optional). While it runs, the button shows progress and cancels. */
function TranscribeButton({ id, title }: { id: string; title: string }) {
  const { mediaId, percent, start } = useTranscribe();
  if (mediaId === id) {
    return (
      <Button title="Stop transcribing" onClick={() => void api.cancelTranscription()}>
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        {percent > 0 ? `${percent}% · Cancel` : "Preparing… · Cancel"}
      </Button>
    );
  }
  return (
    <Button disabled={!!mediaId} title={mediaId ? "Another file is being transcribed" : "Write subtitles (.srt) from the speech, on this computer (whisper.cpp)"} onClick={() => void start(id, title)}>
      <Captions className="h-3.5 w-3.5" />
      Transcribe
    </Button>
  );
}
