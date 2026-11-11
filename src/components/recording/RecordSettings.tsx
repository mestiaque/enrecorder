import { FolderOpen } from "lucide-react";
import type { ReactNode } from "react";
import { api, errorMessage } from "../../lib/api";
import { usePaths, useTools } from "../../lib/queries";
import type { Codec, EncoderStatus, Settings } from "../../lib/types";
import { useSettings } from "../../store/settings";
import { toast, useUi } from "../../store/ui";
import { VENDOR } from "../media/EncoderTable";
import { Panel } from "../ui/blocks";
import { Button, Segmented, Select, Switch } from "../ui/primitives";

const CODEC_LABEL: Record<Codec, string> = { h264: "H.264", hevc: "H.265 (HEVC)", av1: "AV1", vp9: "VP9" };
const ALLOWED: Record<Settings["recFormat"], Codec[]> = { mp4: ["h264", "hevc", "av1"], mkv: ["h264", "hevc", "av1", "vp9"], webm: ["vp9", "av1"] };
/** Same preference order as the backend's "auto". */
const ORDER = ["nvidia", "amd", "intel", "vaapi", "mediaFoundation", "software"];

export function verifiedFor(encoders: EncoderStatus[] | undefined, codec: Codec) {
  return (encoders ?? []).filter((e) => e.codec === codec && e.verified).sort((a, b) => ORDER.indexOf(a.vendor) - ORDER.indexOf(b.vendor));
}

function Line({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-line/60 py-2.5 last:border-0">
      <div className="min-w-0">
        <div className="text-[13px]">{label}</div>
        {hint && <div className="text-[11px] text-faint">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function RecordSettings({ backend }: { backend: string | null | undefined }) {
  const { settings: s, set } = useSettings();
  const { data: tools } = useTools();
  const { data: paths } = usePaths();
  const encoders = tools?.capabilities?.encoders;
  const usable = verifiedFor(encoders, s.recCodec);
  const auto = usable[0];

  const setFormat = (f: Settings["recFormat"]) => {
    void set("recFormat", f);
    if (!ALLOWED[f].includes(s.recCodec)) {
      const next = ALLOWED[f].find((c) => verifiedFor(encoders, c).length) ?? ALLOWED[f][0];
      void set("recCodec", next);
      void set("recEncoder", "auto");
    }
  };

  return (
    <Panel title="Recording settings" bodyClass="py-1">
      <Line label="Frame rate">
        <Segmented value={String(s.recFps)} onChange={(v) => void set("recFps", Number(v) as Settings["recFps"])} options={["24", "30", "60", "120"].map((v) => ({ value: v, label: v }))} />
      </Line>
      <Line label="Resolution" hint="Larger screens are scaled down, never up">
        <Select
          value={s.recResolution}
          onChange={(v) => void set("recResolution", v)}
          options={[
            { value: "native", label: "Native" },
            { value: "720", label: "720p" },
            { value: "1080", label: "1080p" },
            { value: "1440", label: "1440p" },
            { value: "2160", label: "4K" },
          ]}
        />
      </Line>
      <Line label="Format">
        <Select value={s.recFormat} onChange={setFormat} options={[{ value: "mp4", label: "MP4" }, { value: "mkv", label: "MKV" }, { value: "webm", label: "WebM" }]} />
      </Line>
      <Line label="Codec">
        <Select
          value={s.recCodec}
          onChange={(v) => {
            void set("recCodec", v);
            void set("recEncoder", "auto");
          }}
          options={ALLOWED[s.recFormat].map((c) => ({ value: c, label: `${CODEC_LABEL[c]}${encoders && !verifiedFor(encoders, c).length ? " — no encoder" : ""}` }))}
        />
      </Line>
      <Line label="Encoder" hint={!encoders ? "Testing encoders…" : usable.length ? undefined : <span className="text-danger">No encoder for {CODEC_LABEL[s.recCodec]} passed its test</span>}>
        <Select
          value={usable.some((e) => e.name === s.recEncoder) ? s.recEncoder : "auto"}
          onChange={(v) => void set("recEncoder", v)}
          className="max-w-[240px]"
          options={[{ value: "auto", label: auto ? `Automatic (${auto.name})` : "Automatic" }, ...usable.map((e) => ({ value: e.name, label: `${e.name} · ${VENDOR[e.vendor]}` }))]}
        />
      </Line>
      <Line label="Quality">
        <Segmented value={s.recQuality} onChange={(v) => void set("recQuality", v)} options={[{ value: "small", label: "Smaller file" }, { value: "balanced", label: "Balanced" }, { value: "high", label: "High" }]} />
      </Line>
      <Line label="Show the mouse pointer">
        <Switch checked={s.recCursor} onChange={(v) => void set("recCursor", v)} label="Show the mouse pointer" />
      </Line>
      <Line label="Countdown">
        <Select value={s.recCountdown} onChange={(v) => void set("recCountdown", v)} options={[0, 3, 5, 10].map((n) => ({ value: n as Settings["recCountdown"], label: n ? `${n} seconds` : "None" }))} />
      </Line>
      <Line label="Stop automatically">
        <Select value={s.recMaxMinutes} onChange={(v) => void set("recMaxMinutes", v)} options={[0, 5, 10, 30, 60, 120].map((n) => ({ value: n, label: n ? `After ${n} min` : "Never" }))} />
      </Line>
      <Line label="Floating controls" hint={backend === "x11" ? "On X11 they are recorded when they sit inside the recorded area" : "A small bar with the timer, pause and stop"}>
        <Switch checked={s.recShowHud} onChange={(v) => void set("recShowHud", v)} label="Floating controls" />
      </Line>
      <Line label="Hide this window while recording" hint="The tray icon turns red while recording">
        <Switch checked={s.recHideWindow} onChange={(v) => void set("recHideWindow", v)} label="Hide this window while recording" />
      </Line>
      <Line label="Save to" hint={<span className="font-mono">{paths?.recordings ?? "…"}</span>}>
        <div className="flex gap-1.5">
          <Button size="sm" variant="ghost" disabled={!paths} onClick={() => paths && api.openFolder(paths.recordings).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) }))}>
            <FolderOpen className="h-3.5 w-3.5" />
          </Button>
          <Button size="sm" onClick={() => useUi.getState().openDialog("settings", "storage")}>
            Change…
          </Button>
        </div>
      </Line>
    </Panel>
  );
}
