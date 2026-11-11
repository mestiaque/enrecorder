import { useQuery } from "@tanstack/react-query";
import { Clapperboard, Play, Scissors, SkipBack, SkipForward } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { EmptyState, PageHeader, Panel, Row } from "../components/ui/blocks";
import { Button, Select, Slider, Switch } from "../components/ui/primitives";
import { api, errorMessage } from "../lib/api";
import { formatDuration } from "../lib/format";
import { useMedia, useMediaUrl } from "../lib/queries";
import type { ExportPreset } from "../lib/types";
import { useEditorTarget } from "../store/editor";
import { useNav } from "../store/nav";
import { toast } from "../store/ui";

const fmt = (ms: number) => {
  const t = formatDuration(ms);
  return `${t}.${String(Math.floor((ms % 1000) / 100))}`;
};

export function EditorPage() {
  const target = useEditorTarget((s) => s.id);
  const open = useEditorTarget((s) => s.open);
  const library = useMedia({ filter: "all", search: "", sort: "newest", limit: 200 });
  const items = (library.data ?? []).filter((m) => m.kind !== "screenshot");
  const id = target ?? items[0]?.id ?? null;
  const detail = useQuery({ queryKey: ["media-item", id], queryFn: () => api.getMedia(id!), enabled: !!id });
  const item = detail.data?.item;
  const stream = useMediaUrl(id);
  const media = useRef<HTMLVideoElement & HTMLAudioElement>(null);
  const [duration, setDuration] = useState(0);
  const [range, setRange] = useState<[number, number]>([0, 0]);
  const [now, setNow] = useState(0);
  const [volume, setVolume] = useState(100);
  const [mute, setMute] = useState(false);
  const [denoise, setDenoise] = useState(false);
  const [normalize, setNormalize] = useState(false);
  const [format, setFormat] = useState<ExportPreset["format"]>("mp4");

  useEffect(() => {
    const d = item?.durationMs ?? 0;
    setDuration(d);
    setRange([0, d]);
    setNow(0);
    setFormat(item?.kind === "audio" ? "mp3" : "mp4");
  }, [item?.id, item?.durationMs, item?.kind]);

  // Stop at the out point while previewing the selection.
  const onTime = () => {
    const el = media.current;
    if (!el) return;
    const ms = el.currentTime * 1000;
    setNow(ms);
    if (!el.paused && ms >= range[1] && range[1] < duration) el.pause();
  };
  const seek = (ms: number) => {
    if (media.current) media.current.currentTime = ms / 1000;
    setNow(ms);
  };
  const playSelection = () => {
    seek(range[0]);
    void media.current?.play();
  };

  const exportIt = async () => {
    if (!item) return;
    const full = range[0] <= 0 && range[1] >= duration;
    try {
      await api.startExport(item.id, {
        kind: "trim",
        format,
        quality: "high",
        startMs: range[0] > 0 ? Math.round(range[0]) : null,
        endMs: full ? null : Math.round(range[1]),
        removeAudio: mute,
        volume: mute ? 100 : volume,
        denoise: !mute && denoise,
        normalize: !mute && normalize,
      });
      toast({ kind: "success", title: "Export started", detail: `${fmt(range[1] - range[0])} from “${item.title}”`, action: { label: "Export Center", run: () => useNav.getState().go("exports") } });
    } catch (e) {
      toast({ kind: "error", title: "Could not export", detail: errorMessage(e) });
    }
  };

  if (!library.isLoading && !items.length) {
    return (
      <>
        <PageHeader title="Media Editor" subtitle="Trim recordings and change their sound without touching the original." />
        <EmptyState icon={<Clapperboard className="h-6 w-6" />} title="Nothing to edit yet" text="Record a video or audio first; it appears here." action={<Button onClick={() => useNav.getState().go("video")}>Record the screen</Button>} />
      </>
    );
  }

  const src = item && stream.data ? `${stream.data}?v=${item.sizeBytes}` : "";
  return (
    <>
      <PageHeader
        title="Media Editor"
        subtitle="Trim recordings and change their sound without touching the original."
        actions={<Select value={id ?? ""} onChange={(v) => open(v)} className="max-w-[320px]" options={items.map((m) => ({ value: m.id, label: m.title }))} />}
      />
      {item && (
        <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_340px]">
          <Panel bodyClass="p-0 overflow-hidden">
            <div className="bg-black">
              {item.kind === "video" ? (
                <video ref={media} key={src} src={src} className="aspect-video w-full" onTimeUpdate={onTime} onLoadedMetadata={(e) => !duration && setDuration(e.currentTarget.duration * 1000)} controls preload="metadata" />
              ) : (
                <audio ref={media} key={src} src={src} className="w-full p-4" onTimeUpdate={onTime} controls preload="metadata" />
              )}
            </div>
            <div className="space-y-3 p-4">
              <div className="relative h-8">
                {/* Selection bar under the two handles. */}
                <div className="absolute inset-x-0 top-3.5 h-1 rounded-full bg-line" />
                <div className="absolute top-3.5 h-1 rounded-full bg-accent" style={{ left: `${(range[0] / (duration || 1)) * 100}%`, right: `${100 - (range[1] / (duration || 1)) * 100}%` }} />
                <div className="absolute top-2 h-4 w-0.5 bg-fg" style={{ left: `${(now / (duration || 1)) * 100}%` }} />
              </div>
              <Row label="Start" hint={fmt(range[0])}>
                <div className="flex w-[360px] items-center gap-2">
                  <Slider value={range[0]} min={0} max={duration} step={100} onChange={(v) => { const s = Math.min(v, range[1] - 200); setRange([s, range[1]]); seek(s); }} />
                  <Button size="sm" title="Start at the current position" onClick={() => setRange([Math.min(now, range[1] - 200), range[1]])}>
                    <SkipBack className="h-3.5 w-3.5" />
                  </Button>
                </div>
              </Row>
              <Row label="End" hint={fmt(range[1])}>
                <div className="flex w-[360px] items-center gap-2">
                  <Slider value={range[1]} min={0} max={duration} step={100} onChange={(v) => { const e = Math.max(v, range[0] + 200); setRange([range[0], e]); seek(e); }} />
                  <Button size="sm" title="End at the current position" onClick={() => setRange([range[0], Math.max(now, range[0] + 200)])}>
                    <SkipForward className="h-3.5 w-3.5" />
                  </Button>
                </div>
              </Row>
              <div className="flex items-center gap-3">
                <Button onClick={playSelection}>
                  <Play className="h-3.5 w-3.5" />
                  Play selection
                </Button>
                <span className="text-[12px] text-muted">Selection {fmt(range[1] - range[0])} of {fmt(duration)}</span>
              </div>
            </div>
          </Panel>
          <Panel title="Output" bodyClass="py-1">
            <Row label="Remove sound">
              <Switch checked={mute} onChange={setMute} label="Remove sound" />
            </Row>
            {!mute && (
              <Row label="Volume" hint={`${volume}%`}>
                <div className="w-40">
                  <Slider value={volume} min={0} max={300} step={5} onChange={setVolume} />
                </div>
              </Row>
            )}
            {!mute && (
              <Row label="Reduce noise" hint="Hum, fans, hiss">
                <Switch checked={denoise} onChange={setDenoise} label="Reduce noise" />
              </Row>
            )}
            {!mute && (
              <Row label="Normalize loudness" hint="−16 LUFS, the level of podcasts and online video">
                <Switch checked={normalize} onChange={setNormalize} label="Normalize loudness" />
              </Row>
            )}
            <Row label="Format">
              <Select
                value={format}
                onChange={(v) => setFormat(v)}
                options={(item.kind === "video" ? (["mp4", "webm", "mkv", "gif", "mp3", "m4a"] as const) : (["mp3", "m4a", "wav", "flac", "opus"] as const)).map((f) => ({ value: f, label: f.toUpperCase() }))}
              />
            </Row>
            <div className="py-3">
              <Button variant="primary" className="h-10 w-full" onClick={() => void exportIt()} disabled={duration <= 0}>
                <Scissors className="h-4 w-4" />
                Export selection
              </Button>
              <p className="mt-2 text-[11px] text-faint">The original stays as it is; the result is saved in Exports and added to the library.</p>
            </div>
          </Panel>
        </div>
      )}
    </>
  );
}
