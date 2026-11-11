import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import type { ExportPreset, MediaItem } from "../../lib/types";
import { useNav } from "../../store/nav";
import { toast } from "../../store/ui";
import { Dialog } from "../ui/overlays";
import { Button, Segmented, Select, Switch } from "../ui/primitives";

const KINDS: { value: ExportPreset["kind"]; label: string; hint: string }[] = [
  { value: "convert", label: "Convert", hint: "Another format, same quality" },
  { value: "compress", label: "Compress", hint: "Much smaller file for sharing" },
  { value: "audio", label: "Audio only", hint: "Extract the sound" },
  { value: "gif", label: "GIF", hint: "Short animated image" },
];
const VIDEO_FORMATS = ["mp4", "webm", "mkv"] as const;
const AUDIO_FORMATS = ["mp3", "m4a", "wav", "flac", "opus", "ogg"] as const;
const CODECS: Record<string, { value: NonNullable<ExportPreset["videoCodec"]>; label: string }[]> = {
  mp4: [
    { value: "h264", label: "H.264 (plays everywhere)" },
    { value: "hevc", label: "H.265 (smaller)" },
    { value: "av1", label: "AV1 (smallest, slow)" },
  ],
  mkv: [
    { value: "h264", label: "H.264" },
    { value: "hevc", label: "H.265" },
    { value: "av1", label: "AV1" },
    { value: "vp9", label: "VP9" },
  ],
  webm: [
    { value: "vp9", label: "VP9" },
    { value: "av1", label: "AV1" },
  ],
};

function Line({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span>{label}</span>
      {children}
    </div>
  );
}

/** Export with every option; `base` presets fields from the editor (trim, crop, speed…). */
export function ExportDialog({ item, open, onClose, base }: { item: MediaItem; open: boolean; onClose: () => void; base?: Partial<ExportPreset> }) {
  const isVideo = item.kind === "video";
  const [kind, setKind] = useState<ExportPreset["kind"]>(base?.kind ?? (isVideo ? "convert" : "audio"));
  const [format, setFormat] = useState<ExportPreset["format"]>(base?.format ?? (isVideo ? "mp4" : "mp3"));
  const [quality, setQuality] = useState<ExportPreset["quality"]>("balanced");
  const [maxHeight, setMaxHeight] = useState(kind === "compress" ? 720 : 0);
  const [more, setMore] = useState(false);
  const [fps, setFps] = useState(0);
  const [videoCodec, setVideoCodec] = useState<NonNullable<ExportPreset["videoCodec"]>>("");
  const [videoKbps, setVideoKbps] = useState(0);
  const [audioKbps, setAudioKbps] = useState(0);
  const [clicks, setClicks] = useState(false);
  const [keys, setKeys] = useState(false);
  const [track, setTrack] = useState<{ clicks: boolean; keys: boolean } | null>(null);
  const [estimate, setEstimate] = useState<number | null>(null);
  const formats = kind === "gif" ? ["gif"] : kind === "audio" || !isVideo ? AUDIO_FORMATS : VIDEO_FORMATS;
  const audioOut = (AUDIO_FORMATS as readonly string[]).includes(format);

  useEffect(() => {
    if (isVideo) void api.mediaTrack(item.id).then(setTrack).catch(() => setTrack(null));
  }, [item.id, isVideo]);

  const preset: ExportPreset = {
    ...base,
    kind,
    format,
    quality,
    maxHeight,
    volume: base?.volume ?? 100,
    fps,
    videoCodec: VIDEO_FORMATS.includes(format as never) ? videoCodec : "",
    videoKbps,
    audioKbps,
    clickHighlights: clicks,
    keystrokes: keys,
  };
  const key = JSON.stringify(preset);
  useEffect(() => {
    const t = window.setTimeout(() => void api.estimateExport(item.id, preset).then(setEstimate).catch(() => setEstimate(null)), 250);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, item.id]);

  const choose = (k: ExportPreset["kind"]) => {
    setKind(k);
    setFormat(k === "gif" ? "gif" : k === "audio" ? "mp3" : "mp4");
    setMaxHeight(k === "compress" ? 720 : k === "gif" ? 480 : 0);
    setVideoCodec("");
  };

  const start = async () => {
    try {
      await api.startExport(item.id, preset);
      toast({ kind: "success", title: "Export started", detail: "Follow it in the Export Center", action: { label: "Open", run: () => useNav.getState().go("exports") } });
      onClose();
    } catch (e) {
      toast({ kind: "error", title: "Could not start the export", detail: errorMessage(e) });
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={`Export “${item.title}”`}
      description="The original stays untouched; the result is saved in the Exports folder and the library."
      footer={
        <>
          <span className="mr-auto text-[12px] text-muted">{estimate != null ? `About ${formatBytes(estimate)}` : ""}</span>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={() => void start()}>
            Export
          </Button>
        </>
      }
    >
      <div className="space-y-4 text-[13px]">
        {!base?.kind && (
          <div className="grid grid-cols-2 gap-2">
            {KINDS.filter((k) => isVideo || k.value === "audio").map((k) => (
              <button key={k.value} onClick={() => choose(k.value)} className={`rounded-lg border p-3 text-left ${kind === k.value ? "border-accent bg-accent/10" : "border-line hover:border-faint"}`}>
                <div className="font-medium">{k.label}</div>
                <div className="text-[11px] text-faint">{k.hint}</div>
              </button>
            ))}
          </div>
        )}
        <Line label="Format">
          <Select
            value={format}
            onChange={(v) => {
              setFormat(v as ExportPreset["format"]);
              setVideoCodec("");
            }}
            options={(base?.kind ? [...VIDEO_FORMATS, ...AUDIO_FORMATS, "gif"].filter((f) => isVideo || (AUDIO_FORMATS as readonly string[]).includes(f)) : formats).map((f) => ({ value: f, label: f.toUpperCase() }))}
          />
        </Line>
        {format !== "wav" && format !== "flac" && (
          <Line label="Quality">
            <Segmented value={quality} onChange={(v) => setQuality(v as ExportPreset["quality"])} options={[{ value: "small", label: "Smaller" }, { value: "balanced", label: "Balanced" }, { value: "high", label: "High" }]} />
          </Line>
        )}
        {isVideo && !audioOut && (
          <Line label="Size">
            <Select value={maxHeight} onChange={setMaxHeight} options={[{ value: 0, label: "Original" }, { value: 2160, label: "4K" }, { value: 1440, label: "1440p" }, { value: 1080, label: "1080p" }, { value: 720, label: "720p" }, { value: 480, label: "480p" }, { value: 360, label: "360p" }]} />
          </Line>
        )}
        {isVideo && !audioOut && track && (
          <div className="space-y-2 rounded-lg border border-line p-3">
            <div className="text-[11px] text-faint">Added to the exported file from what was remembered while recording (post-processing; the recording itself has none).</div>
            <Line label="Highlight the pointer and clicks">
              <Switch checked={clicks} onChange={setClicks} label="Highlight the pointer and clicks" />
            </Line>
            {track.keys && (
              <Line label="Show key presses">
                <Switch checked={keys} onChange={setKeys} label="Show key presses" />
              </Line>
            )}
          </div>
        )}
        <button className="text-[12px] text-accent hover:underline" onClick={() => setMore(!more)}>
          {more ? "Fewer options" : "More options…"}
        </button>
        {more && (
          <div className="space-y-3">
            {isVideo && !audioOut && format !== "gif" && (
              <>
                <Line label="Video codec">
                  <Select value={videoCodec} onChange={setVideoCodec} options={[{ value: "" as const, label: "Default" }, ...(CODECS[format] ?? [])]} />
                </Line>
                <Line label="Frame rate">
                  <Select value={fps} onChange={setFps} options={[{ value: 0, label: "Keep" }, { value: 24, label: "24 fps" }, { value: 30, label: "30 fps" }, { value: 60, label: "60 fps" }]} />
                </Line>
                <Line label="Video bitrate">
                  <Select value={videoKbps} onChange={setVideoKbps} options={[{ value: 0, label: "From quality" }, { value: 1000, label: "1 Mbit/s" }, { value: 2500, label: "2.5 Mbit/s" }, { value: 5000, label: "5 Mbit/s" }, { value: 8000, label: "8 Mbit/s" }, { value: 16000, label: "16 Mbit/s" }]} />
                </Line>
              </>
            )}
            {!["wav", "flac", "gif", "ogg"].includes(format) && (
              <Line label="Audio bitrate">
                <Select value={audioKbps} onChange={setAudioKbps} options={[{ value: 0, label: "From quality" }, { value: 64, label: "64 kbit/s" }, { value: 128, label: "128 kbit/s" }, { value: 192, label: "192 kbit/s" }, { value: 320, label: "320 kbit/s" }]} />
              </Line>
            )}
          </div>
        )}
      </div>
    </Dialog>
  );
}
