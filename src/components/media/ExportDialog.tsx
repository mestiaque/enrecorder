import { useState } from "react";
import { api, errorMessage } from "../../lib/api";
import type { ExportPreset, MediaItem } from "../../lib/types";
import { useNav } from "../../store/nav";
import { toast } from "../../store/ui";
import { Dialog } from "../ui/overlays";
import { Button, Segmented, Select } from "../ui/primitives";

const KINDS: { value: ExportPreset["kind"]; label: string; hint: string }[] = [
  { value: "convert", label: "Convert", hint: "Another format, same quality" },
  { value: "compress", label: "Compress", hint: "Much smaller file for sharing" },
  { value: "audio", label: "Audio only", hint: "Extract the sound" },
  { value: "gif", label: "GIF", hint: "Short animated image" },
];
const VIDEO_FORMATS = ["mp4", "webm", "mkv"] as const;
const AUDIO_FORMATS = ["mp3", "m4a", "wav", "flac", "opus"] as const;

export function ExportDialog({ item, open, onClose }: { item: MediaItem; open: boolean; onClose: () => void }) {
  const isVideo = item.kind === "video";
  const [kind, setKind] = useState<ExportPreset["kind"]>(isVideo ? "convert" : "audio");
  const [format, setFormat] = useState<ExportPreset["format"]>(isVideo ? "mp4" : "mp3");
  const [quality, setQuality] = useState<ExportPreset["quality"]>("balanced");
  const [maxHeight, setMaxHeight] = useState(kind === "compress" ? 720 : 0);
  const formats = kind === "gif" ? ["gif"] : kind === "audio" || !isVideo ? AUDIO_FORMATS : VIDEO_FORMATS;

  const choose = (k: ExportPreset["kind"]) => {
    setKind(k);
    setFormat(k === "gif" ? "gif" : k === "audio" ? "mp3" : "mp4");
    setMaxHeight(k === "compress" ? 720 : k === "gif" ? 480 : 0);
  };

  const start = async () => {
    try {
      await api.startExport(item.id, { kind, format, quality, maxHeight, volume: 100 });
      toast({ kind: "success", title: "Export started", detail: "Follow it in the Export Center", action: { label: "Open", run: () => useNav.getState().go("exports") } });
      onClose();
    } catch (e) {
      toast({ kind: "error", title: "Could not start the export", detail: errorMessage(e) });
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()} title={`Export “${item.title}”`} description="The original stays untouched; the result is saved in the Exports folder and the library." footer={<><Button onClick={onClose}>Cancel</Button><Button variant="primary" onClick={() => void start()}>Export</Button></>}>
      <div className="space-y-4 text-[13px]">
        <div className="grid grid-cols-2 gap-2">
          {KINDS.filter((k) => isVideo || k.value === "audio").map((k) => (
            <button key={k.value} onClick={() => choose(k.value)} className={`rounded-lg border p-3 text-left ${kind === k.value ? "border-accent bg-accent/10" : "border-line hover:border-faint"}`}>
              <div className="font-medium">{k.label}</div>
              <div className="text-[11px] text-faint">{k.hint}</div>
            </button>
          ))}
        </div>
        <div className="flex items-center justify-between">
          <span>Format</span>
          <Select value={format} onChange={(v) => setFormat(v as ExportPreset["format"])} options={formats.map((f) => ({ value: f, label: f.toUpperCase() }))} />
        </div>
        {format !== "wav" && format !== "flac" && (
          <div className="flex items-center justify-between">
            <span>Quality</span>
            <Segmented value={quality} onChange={(v) => setQuality(v as ExportPreset["quality"])} options={[{ value: "small", label: "Smaller" }, { value: "balanced", label: "Balanced" }, { value: "high", label: "High" }]} />
          </div>
        )}
        {(kind === "compress" || kind === "convert" || kind === "gif") && isVideo && (
          <div className="flex items-center justify-between">
            <span>Size</span>
            <Select value={maxHeight} onChange={setMaxHeight} options={[{ value: 0, label: "Original" }, { value: 1080, label: "1080p" }, { value: 720, label: "720p" }, { value: 480, label: "480p" }, { value: 360, label: "360p" }]} />
          </div>
        )}
      </div>
    </Dialog>
  );
}
