import { useQuery } from "@tanstack/react-query";
import { Camera, Clapperboard, Crop, Play, RotateCw, Scissors, SkipBack, SkipForward, Split, VolumeX, Wand2 } from "lucide-react";
import { ExportDialog } from "../components/media/ExportDialog";
import { useEffect, useRef, useState } from "react";
import { EmptyState, PageHeader, Panel, Row } from "../components/ui/blocks";
import { Button, Segmented, Select, Slider, Switch } from "../components/ui/primitives";
import { api, errorMessage } from "../lib/api";
import { formatBytes, formatDuration } from "../lib/format";
import { useQueryClient } from "@tanstack/react-query";
import { useMedia, useMediaUrl } from "../lib/queries";
import type { ExportPreset } from "../lib/types";
import { useEditorTarget } from "../store/editor";
import { convertFileSrc } from "@tauri-apps/api/core";
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
  const [rotate, setRotate] = useState<0 | 90 | 180 | 270>(0);
  const [speed, setSpeed] = useState(1);
  const [fadeIn, setFadeIn] = useState(0);
  const [fadeOut, setFadeOut] = useState(0);
  const [maxHeight, setMaxHeight] = useState(0);
  const [crop, setCrop] = useState<ExportPreset["crop"]>(null);
  const [cropping, setCropping] = useState(false);
  const [drag, setDrag] = useState<{ x: number; y: number } | null>(null);
  const [silence, setSilence] = useState<[number, number | null][] | null>(null);
  const [track, setTrack] = useState<{ clicks: boolean; keys: boolean } | null>(null);
  const [clicks, setClicks] = useState(false);
  const [keys, setKeys] = useState(false);
  const [estimate, setEstimate] = useState<number | null>(null);
  const [more, setMore] = useState(false);
  const [wave, setWave] = useState<string | null>(null);
  const qc = useQueryClient();

  useEffect(() => {
    const d = item?.durationMs ?? 0;
    setDuration(d);
    setRange([0, d]);
    setNow(0);
    setFormat(item?.kind === "audio" ? "mp3" : "mp4");
    setCrop(null);
    setRotate(0);
    setSpeed(1);
    setSilence(null);
    setClicks(false);
    setKeys(false);
    setWave(null);
    if (item?.kind === "video") void api.mediaTrack(item.id).then(setTrack).catch(() => setTrack(null));
    else setTrack(null);
    // The audio waveform (the library thumbnail of an audio item) under the timeline.
    if (item?.kind === "audio") void api.mediaThumbnail(item.id).then((p) => setWave(convertFileSrc(p))).catch(() => setWave(null));
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

  const presetFor = (a: number, b: number): ExportPreset => {
    const full = a <= 0 && b >= duration;
    const video = item?.kind === "video";
    return {
      kind: "trim",
      format,
      quality: "high",
      startMs: a > 0 ? Math.round(a) : null,
      endMs: full ? null : Math.round(b),
      removeAudio: mute,
      volume: mute ? 100 : volume,
      denoise: !mute && denoise,
      normalize: !mute && normalize,
      rotate: video ? rotate : 0,
      speed,
      fadeInMs: fadeIn,
      fadeOutMs: fadeOut,
      maxHeight: video ? maxHeight : 0,
      crop: video ? crop : null,
      clickHighlights: video && clicks,
      keystrokes: video && keys,
    };
  };
  const preset = presetFor(range[0], range[1]);
  const presetKey = JSON.stringify(preset);
  useEffect(() => {
    if (!item) return;
    const t = window.setTimeout(() => void api.estimateExport(item.id, preset).then(setEstimate).catch(() => setEstimate(null)), 300);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [presetKey, item?.id]);

  const run = async (p: ExportPreset, what: string) => {
    if (!item) return;
    try {
      await api.startExport(item.id, p);
      toast({ kind: "success", title: "Export started", detail: what, action: { label: "Export Center", run: () => useNav.getState().go("exports") } });
    } catch (e) {
      toast({ kind: "error", title: "Could not export", detail: errorMessage(e) });
    }
  };
  const exportIt = () => run(preset, `${fmt(range[1] - range[0])} from “${item?.title}”`);
  // Split: two files, before and after the playhead (inside the selection).
  const split = async () => {
    if (now <= range[0] + 200 || now >= range[1] - 200) {
      toast({ kind: "info", title: "Move the playhead inside the selection to split there" });
      return;
    }
    await run(presetFor(range[0], now), `Part 1 (${fmt(now - range[0])})`);
    await run(presetFor(now, range[1]), `Part 2 (${fmt(range[1] - now)})`);
  };
  const saveFrame = () =>
    item &&
    void api
      .extractFrame(item.id, Math.round(now))
      .then(() => {
        toast({ kind: "success", title: "Frame saved", detail: "It is in the library with your screenshots." });
        void qc.invalidateQueries({ queryKey: ["media"] });
      })
      .catch((e) => toast({ kind: "error", title: "Could not save the frame", detail: errorMessage(e) }));
  const findSilence = () =>
    item &&
    void api
      .detectSilence(item.id)
      .then((r) => {
        setSilence(r);
        if (!r.length) toast({ kind: "info", title: "No silent parts longer than 0.7 s" });
      })
      .catch((e) => toast({ kind: "error", title: "Could not analyse the sound", detail: errorMessage(e) }));
  // Trim to the sound: from the end of a leading silence to the start of a trailing one.
  const trimSilence = () => {
    if (!silence?.length) return;
    let a = range[0];
    let b = range[1];
    const first = silence[0];
    if (first[0] <= 100 && first[1] != null) a = Math.max(a, first[1] - 150);
    const last = silence[silence.length - 1];
    if (last[1] == null || last[1] >= duration - 100) b = Math.min(b, last[0] + 150);
    if (b > a + 200) setRange([a, b]);
  };
  // Crop drawing on the preview: CSS pixels → source pixels.
  const toSource = (cx: number, cy: number) => {
    const el = media.current;
    if (!el || !el.videoWidth) return null;
    const r = el.getBoundingClientRect();
    // object-fit: contain inside the element.
    const scale = Math.min(r.width / el.videoWidth, r.height / el.videoHeight);
    const ox = (r.width - el.videoWidth * scale) / 2;
    const oy = (r.height - el.videoHeight * scale) / 2;
    return { x: Math.max(0, Math.min(el.videoWidth, (cx - r.left - ox) / scale)), y: Math.max(0, Math.min(el.videoHeight, (cy - r.top - oy) / scale)) };
  };
  const cropStyle = () => {
    const el = media.current;
    if (!el || !crop || !el.videoWidth) return undefined;
    const r = el.getBoundingClientRect();
    const scale = Math.min(r.width / el.videoWidth, r.height / el.videoHeight);
    const ox = (r.width - el.videoWidth * scale) / 2;
    const oy = (r.height - el.videoHeight * scale) / 2;
    return { left: ox + crop.x * scale, top: oy + crop.y * scale, width: crop.width * scale, height: crop.height * scale };
  };

  if (!library.isLoading && !items.length) {
    return (
      <>
        <PageHeader title="Media Editor" subtitle="Trim, split, crop, rotate, change speed and sound — the original stays untouched." />
        <EmptyState icon={<Clapperboard className="h-6 w-6" />} title="Nothing to edit yet" text="Record a video or audio first; it appears here." action={<Button onClick={() => useNav.getState().go("video")}>Record the screen</Button>} />
      </>
    );
  }

  const src = item && stream.data ? `${stream.data}?v=${item.sizeBytes}` : "";
  return (
    <>
      <PageHeader
        title="Media Editor"
        subtitle="Trim, split, crop, rotate, change speed and sound — the original stays untouched."
        actions={<Select value={id ?? ""} onChange={(v) => open(v)} className="max-w-[320px]" options={items.map((m) => ({ value: m.id, label: m.title }))} />}
      />
      {item && (
        <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_340px]">
          <Panel bodyClass="p-0 overflow-hidden">
            <div className="bg-black">
              {item.kind === "video" ? (
                <div className="relative">
                  <video ref={media} key={src} src={src} className="aspect-video w-full" onTimeUpdate={onTime} onLoadedMetadata={(e) => !duration && setDuration(e.currentTarget.duration * 1000)} controls={!cropping} preload="metadata" />
                  {cropping && (
                    <div
                      className="absolute inset-0 cursor-crosshair"
                      onMouseDown={(e) => {
                        const p = toSource(e.clientX, e.clientY);
                        if (p) {
                          setDrag(p);
                          setCrop(null);
                        }
                      }}
                      onMouseMove={(e) => {
                        if (!drag) return;
                        const p = toSource(e.clientX, e.clientY);
                        if (p) setCrop({ x: Math.round(Math.min(drag.x, p.x)), y: Math.round(Math.min(drag.y, p.y)), width: Math.round(Math.abs(p.x - drag.x)), height: Math.round(Math.abs(p.y - drag.y)) });
                      }}
                      onMouseUp={() => {
                        setDrag(null);
                        if (crop && (crop.width < 16 || crop.height < 16)) setCrop(null);
                      }}
                    />
                  )}
                  {crop && <div className="pointer-events-none absolute border-2 border-accent" style={{ ...cropStyle(), boxShadow: "0 0 0 9999px rgba(0,0,0,0.45)" }} />}
                </div>
              ) : (
                <audio ref={media} key={src} src={src} className="w-full p-4" onTimeUpdate={onTime} controls preload="metadata" />
              )}
            </div>
            <div className="space-y-3 p-4">
              <div className="relative h-8 overflow-hidden rounded" style={wave ? { backgroundImage: `url(${wave})`, backgroundSize: "100% 100%" } : undefined}>
                {/* Silent parts found by "Find silence". */}
                {(silence ?? []).map(([a, b], i) => (
                  <div key={i} className="absolute inset-y-0 bg-faint/30" style={{ left: `${(a / (duration || 1)) * 100}%`, width: `${(((b ?? duration) - a) / (duration || 1)) * 100}%` }} title="Silence" />
                ))}
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
                <Button title="Two files: before and after the playhead" onClick={() => void split()}>
                  <Split className="h-3.5 w-3.5" />
                  Split here
                </Button>
                {item.kind === "video" && (
                  <Button title="Save this frame as a screenshot" onClick={saveFrame}>
                    <Camera className="h-3.5 w-3.5" />
                    Save frame
                  </Button>
                )}
                <Button title="Mark the silent parts on the timeline" onClick={findSilence}>
                  <VolumeX className="h-3.5 w-3.5" />
                  Find silence
                </Button>
                {silence && silence.length > 0 && (
                  <Button title="Cut silence at the start and the end" onClick={trimSilence}>
                    <Wand2 className="h-3.5 w-3.5" />
                    Trim silence
                  </Button>
                )}
                <span className="text-[12px] text-muted">Selection {fmt(range[1] - range[0])} of {fmt(duration)}</span>
              </div>
            </div>
          </Panel>
          <Panel title="Output" bodyClass="py-1">
            {item.kind === "video" && (
              <>
                <Row label="Crop" hint={crop ? `${crop.width}×${crop.height} at ${crop.x}, ${crop.y}` : "Drag on the picture to choose an area"}>
                  <div className="flex gap-1.5">
                    {crop && (
                      <Button size="sm" variant="ghost" onClick={() => setCrop(null)}>
                        Reset
                      </Button>
                    )}
                    <Button size="sm" variant={cropping ? "primary" : "default"} onClick={() => setCropping(!cropping)}>
                      <Crop className="h-3.5 w-3.5" />
                      {cropping ? "Done" : "Crop"}
                    </Button>
                  </div>
                </Row>
                <Row label="Rotate">
                  <Segmented value={String(rotate)} onChange={(v) => setRotate(Number(v) as 0 | 90 | 180 | 270)} options={[{ value: "0", label: "0°" }, { value: "90", label: <><RotateCw className="h-3 w-3" />90°</> }, { value: "180", label: "180°" }, { value: "270", label: "270°" }]} />
                </Row>
                <Row label="Size">
                  <Select value={maxHeight} onChange={setMaxHeight} options={[{ value: 0, label: "Original" }, { value: 1080, label: "1080p" }, { value: 720, label: "720p" }, { value: 480, label: "480p" }]} />
                </Row>
                {track && (
                  <>
                    <Row label="Highlight clicks" hint="From the pointer track saved while recording">
                      <Switch checked={clicks} onChange={setClicks} label="Highlight clicks" />
                    </Row>
                    {track.keys && (
                      <Row label="Show key presses">
                        <Switch checked={keys} onChange={setKeys} label="Show key presses" />
                      </Row>
                    )}
                  </>
                )}
              </>
            )}
            <Row label="Speed" hint="Sound keeps its pitch">
              <Select value={speed} onChange={setSpeed} options={[0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4].map((v) => ({ value: v, label: `${v}×` }))} />
            </Row>
            <Row label="Fade in / out">
              <div className="flex gap-1.5">
                <Select value={fadeIn} onChange={setFadeIn} options={[0, 500, 1000, 2000, 3000].map((v) => ({ value: v, label: v ? `In ${v / 1000} s` : "No fade in" }))} />
                <Select value={fadeOut} onChange={setFadeOut} options={[0, 500, 1000, 2000, 3000].map((v) => ({ value: v, label: v ? `Out ${v / 1000} s` : "No fade out" }))} />
              </div>
            </Row>
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
                options={(item.kind === "video" ? (["mp4", "webm", "mkv", "gif", "mp3", "m4a"] as const) : (["mp3", "m4a", "wav", "flac", "opus", "ogg"] as const)).map((f) => ({ value: f, label: f.toUpperCase() }))}
              />
            </Row>
            <div className="py-3">
              <Button variant="primary" className="h-10 w-full" onClick={() => void exportIt()} disabled={duration <= 0}>
                <Scissors className="h-4 w-4" />
                Export selection
              </Button>
              <p className="mt-2 text-[11px] text-faint">
                {estimate != null ? `About ${formatBytes(estimate)}. ` : ""}The original stays as it is; the result is saved in Exports and added to the library.
              </p>
              <Button variant="ghost" size="sm" className="mt-1" onClick={() => setMore(true)}>
                Codec, frame rate, bitrate…
              </Button>
            </div>
          </Panel>
        </div>
      )}
      {item && more && <ExportDialog item={item} open onClose={() => setMore(false)} base={preset} />}
    </>
  );
}
