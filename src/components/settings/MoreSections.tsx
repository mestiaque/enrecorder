/** Settings sections: Screenshots, Performance, Privacy, Shortcuts, Devices and Transcription (in Tools). */
import { useQuery } from "@tanstack/react-query";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Keyboard, RefreshCw, RotateCcw } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, errorMessage } from "../../lib/api";
import { cn, formatDate } from "../../lib/format";
import type { DeviceRow, Settings, ShortcutRow } from "../../lib/types";
import { useSettings } from "../../store/settings";
import { toast } from "../../store/ui";
import { Button, Input, Select, Slider, Switch } from "../ui/primitives";

function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-line/60 py-3 last:border-0">
      <div className="min-w-0">
        <div className="text-[13px]">{label}</div>
        {hint && <div className="mt-0.5 break-words text-[11px] text-faint">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

type BoolKey = { [K in keyof Settings]: Settings[K] extends boolean ? K : never }[keyof Settings];
function Toggle({ k, label, hint }: { k: BoolKey; label: string; hint?: ReactNode }) {
  const { settings, set } = useSettings();
  return (
    <Field label={label} hint={hint}>
      <Switch checked={settings[k]} onChange={(v) => void set(k, v)} label={label} />
    </Field>
  );
}

export function ScreenshotSection() {
  const { settings: s, set } = useSettings();
  const [template, setTemplate] = useState(s.shotNameTemplate);
  useEffect(() => setTemplate(s.shotNameTemplate), [s.shotNameTemplate]);
  const now = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  const preview = template.replace("{date}", `${now.getFullYear()}-${p(now.getMonth() + 1)}-${p(now.getDate())}`).replace("{time}", `${p(now.getHours())}-${p(now.getMinutes())}-${p(now.getSeconds())}`).replace("{mode}", "screen");
  return (
    <>
      <Field label="Default format" hint="Used when a new screenshot is saved from the editor">
        <Select value={s.shotFormat} onChange={(v) => void set("shotFormat", v)} options={[{ value: "png", label: "PNG (lossless)" }, { value: "jpeg", label: "JPEG (small)" }, { value: "webp", label: "WebP (small, sharp)" }]} />
      </Field>
      <Field label="JPEG / WebP quality" hint={`${s.shotQuality} %`}>
        <div className="w-44">
          <Slider value={s.shotQuality} min={50} max={100} step={5} onChange={(v) => useSettings.setState({ settings: { ...s, shotQuality: v } })} onCommit={(v) => void set("shotQuality", v)} />
        </div>
      </Field>
      <Field label="File name" hint={<>{"{date}, {time} and {mode} are filled in. Example: "}<span className="font-mono">{preview}.png</span></>}>
        <Input className="w-64" value={template} onChange={(e) => setTemplate(e.target.value)} onBlur={() => void set("shotNameTemplate", template)} onKeyDown={(e) => e.key === "Enter" && void set("shotNameTemplate", template)} />
      </Field>
      <div className="py-3 text-[11px] text-faint">Scrolling capture is not offered: other applications cannot be scrolled and stitched reliably from outside, and a stitched image with gaps or repeats would be misleading.</div>
    </>
  );
}

export function PerformanceSection() {
  return (
    <>
      <Toggle k="hwAccel" label="Use the graphics card for encoding" hint="With the encoder on “Automatic”, verified GPU encoders (VA-API, NVENC, Quick Sync, AMF) are preferred. Turn off if recordings stutter or look wrong." />
      <Toggle k="thumbnails" label="Make thumbnails for the library" hint="Small preview images (cached in the data folder). Off saves a little work on large libraries." />
      <Field label="Exports" hint="One export runs at a time, so recording stays smooth; the others wait in the queue.">
        <span className="text-[12px] text-muted">One at a time</span>
      </Field>
    </>
  );
}

export function PrivacySection() {
  const { settings: s, set } = useSettings();
  return (
    <>
      <div className="py-3 text-[12px] text-muted">ENRecorder works offline. Recordings, screenshots, tracks and logs stay on this computer; there is no account, no upload and no telemetry. The only network request is the optional update check (Settings → General).</div>
      <Toggle k="notifyOnSave" label="Notifications" hint="A desktop notification when a recording, an export or a transcript is saved, or a recording fails" />
      <Field label="Keep logs for" hint="Older log files are deleted when ENRecorder starts">
        <Select value={s.logRetentionDays} onChange={(v) => void set("logRetentionDays", v)} options={[{ value: 3, label: "3 days" }, { value: 7, label: "7 days" }, { value: 14, label: "14 days" }, { value: 30, label: "30 days" }, { value: 0, label: "Forever" }]} />
      </Field>
      <Field label="Key presses" hint="Only remembered when “Remember key presses” is on in the Video Recorder, kept in the data folder and deleted with the recording">
        <Switch checked={s.recTrackKeys} onChange={(v) => void set("recTrackKeys", v)} label="Remember key presses" />
      </Field>
    </>
  );
}

/** Turns a key event into "Ctrl+Shift+R" (null while only modifiers are down). */
function accel(e: React.KeyboardEvent): string | null {
  const k = e.key.length === 1 ? e.key.toUpperCase() : /^F([1-9]|1[0-2])$/.test(e.key) ? e.key : null;
  if (!k || !/^[A-Z0-9]$|^F\d+$/.test(k)) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
  return [...mods, k].join("+");
}

export function ShortcutsSection() {
  const [rows, setRows] = useState<ShortcutRow[]>([]);
  const [recording, setRecording] = useState<string | null>(null);
  useEffect(() => {
    void api.getShortcuts().then(setRows).catch((e) => toast({ kind: "error", title: "Could not read the shortcuts", detail: errorMessage(e) }));
  }, []);
  const save = async (action: string, value: string | null) => {
    try {
      setRows(await api.setShortcut(action, value));
      setRecording(null);
    } catch (e) {
      toast({ kind: "error", title: "Shortcut not changed", detail: errorMessage(e) }, 8000);
    }
  };
  return (
    <>
      <div className="py-3 text-[11px] text-faint">Global shortcuts work while ENRecorder is hidden. Click a shortcut, then press the new keys (with Ctrl, Alt or Super). A shortcut another action or program already uses is refused.</div>
      {rows.map((r) => (
        <Field key={r.action} label={r.label} hint={r.accelerator !== r.default ? `Default: ${r.default}` : undefined}>
          <div className="flex items-center gap-1.5">
            <button
              onClick={() => setRecording(r.action)}
              onKeyDown={(e) => {
                if (recording !== r.action) return;
                e.preventDefault();
                e.stopPropagation();
                if (e.key === "Escape") return setRecording(null);
                const a = accel(e);
                if (a) void save(r.action, a);
              }}
              onBlur={() => recording === r.action && setRecording(null)}
              className={cn("flex h-8 min-w-[150px] items-center justify-center gap-1.5 rounded-md border px-3 font-mono text-[12px]", recording === r.action ? "border-accent text-accent" : "border-line bg-panel")}
            >
              <Keyboard className="h-3.5 w-3.5" />
              {recording === r.action ? "Press keys…" : r.accelerator ?? "Off"}
            </button>
            <Button size="sm" variant="ghost" title="Turn off" disabled={!r.accelerator} onClick={() => void save(r.action, null)}>
              Off
            </Button>
            <Button size="sm" variant="ghost" title="Back to the default" disabled={r.accelerator === r.default} onClick={() => void save(r.action, r.default)}>
              <RotateCcw className="h-3.5 w-3.5" />
            </Button>
          </div>
        </Field>
      ))}
      <div className="flex justify-end py-3">
        <Button size="sm" onClick={() => void api.resetShortcuts().then(setRows).catch((e) => toast({ kind: "error", title: "Could not reset", detail: errorMessage(e) }))}>
          Reset all shortcuts
        </Button>
      </div>
    </>
  );
}

const KIND: Record<DeviceRow["kind"], string> = { microphone: "Microphones", "system-audio": "System audio", webcam: "Cameras", display: "Displays", speaker: "Speakers" };

export function DevicesSection() {
  const q = useQuery({ queryKey: ["devices"], queryFn: api.deviceManager });
  const rows = q.data ?? [];
  const kinds = [...new Set(rows.map((r) => r.kind))];
  return (
    <div className="space-y-4 py-3">
      <div className="flex items-center gap-2">
        <div className="flex-1 text-[11px] text-faint">Devices ENRecorder can record from. Unplugged ones stay listed (greyed) so you can see what was used before.</div>
        <Button size="sm" variant="ghost" onClick={() => void q.refetch()}>
          <RefreshCw className={cn("h-3.5 w-3.5", q.isFetching && "animate-spin")} />
          Refresh
        </Button>
      </div>
      {q.isError && <div className="text-[12px] text-danger">{errorMessage(q.error)}</div>}
      {kinds.map((k) => (
        <section key={k}>
          <div className="mb-1 text-[11px] font-semibold uppercase tracking-wider text-faint">{KIND[k] ?? k}</div>
          <div className="divide-y divide-line rounded-lg bg-raised px-3">
            {rows
              .filter((r) => r.kind === k)
              .map((r) => (
                <div key={r.backendId} className={cn("flex items-center justify-between gap-4 py-2 text-[12px]", !r.connected && "opacity-50")}>
                  <span className="min-w-0">
                    <span className="block truncate">
                      {r.name}
                      {r.isDefault && <span className="ml-1.5 rounded bg-accent/15 px-1 text-[10px] text-accent">Default</span>}
                    </span>
                    <span className="block truncate font-mono text-[10px] text-faint">{r.backendId}</span>
                  </span>
                  <span className="shrink-0 text-right text-faint">
                    {r.details.sampleRate ? `${r.details.sampleRate / 1000} kHz · ${r.details.channels === 1 ? "mono" : `${r.details.channels} ch`}` : r.details.width ? `${r.details.width}×${r.details.height}` : ""}
                    <span className={cn("ml-2", r.connected ? "text-success" : "")}>{r.connected ? "Connected" : `Last seen ${formatDate(r.lastSeen)}`}</span>
                  </span>
                </div>
              ))}
          </div>
        </section>
      ))}
      {!q.isLoading && !rows.length && <div className="text-[12px] text-muted">No devices found.</div>}
    </div>
  );
}

const LANGUAGES = [
  { value: "auto", label: "Detect" },
  { value: "en", label: "English" },
  { value: "bn", label: "Bengali" },
  { value: "hi", label: "Hindi" },
  { value: "ar", label: "Arabic" },
  { value: "es", label: "Spanish" },
  { value: "fr", label: "French" },
  { value: "de", label: "German" },
];

const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

/** Optional transcription with whisper.cpp (Settings → Tools). */
export function TranscriptionSection() {
  const { settings, set } = useSettings();
  const { data: st, refetch } = useQuery({ queryKey: ["transcription"], queryFn: api.transcriptionStatus });
  const choose = async (key: "whisperPath" | "whisperModel", title: string) => {
    const p = await openDialog({ title, multiple: false, directory: false, filters: key === "whisperModel" ? [{ name: "whisper.cpp model", extensions: ["bin"] }] : undefined });
    if (typeof p === "string") {
      await set(key, p);
      void refetch();
    }
  };
  const model = st?.models[0];
  const ready = !!st?.program && !!model;
  return (
    <div>
      <div className="mb-1 mt-2 text-[11px] font-semibold uppercase tracking-wider text-faint">Transcription (optional)</div>
      <p className="mb-1 text-[11px] text-faint">
        Turns speech in a recording into subtitles (an .srt file next to it) with whisper.cpp, on this computer. ENRecorder does not include it: install the
        whisper.cpp program and put a speech model (ggml-base.bin, ggml-small.bin…) in the models folder, or choose them below.
      </p>
      <Field label="whisper.cpp" hint={st?.program ? <span className="font-mono">{st.program}</span> : <span className="text-danger">Not found (looked for whisper-cli on PATH and in the models folder)</span>}>
        <div className="flex gap-2">
          {settings.whisperPath && (
            <Button size="sm" variant="ghost" onClick={() => void set("whisperPath", "").then(() => refetch())}>
              Automatic
            </Button>
          )}
          <Button size="sm" onClick={() => void choose("whisperPath", "Choose the whisper.cpp program (whisper-cli)")}>
            Choose…
          </Button>
        </div>
      </Field>
      <Field label="Speech model" hint={model ? <span className="font-mono">{fileName(model)}</span> : <span className="text-danger">No ggml-*.bin model found</span>}>
        <div className="flex gap-2">
          {settings.whisperModel && (
            <Button size="sm" variant="ghost" onClick={() => void set("whisperModel", "").then(() => refetch())}>
              Automatic
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={() => st && void api.openFolder(st.modelFolder).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) }))}>
            Models folder
          </Button>
          <Button size="sm" onClick={() => void choose("whisperModel", "Choose a whisper.cpp model")}>
            Choose…
          </Button>
        </div>
      </Field>
      <Field label="Spoken language" hint={model && /\.en[.-]/.test(fileName(model)) ? "This model only understands English" : "Detect works for most recordings; choosing the language is faster and more accurate"}>
        <Select className="w-36" value={settings.transcribeLanguage} onChange={(v) => void set("transcribeLanguage", v)} options={LANGUAGES} />
      </Field>
      <Field label="Status" hint={ready ? "Use Transcribe in a recording's details." : "Transcribe stays unavailable until both are found."}>
        <span className={cn("text-[12px]", ready ? "text-success" : "text-faint")}>{ready ? "Ready" : "Not set up"}</span>
      </Field>
    </div>
  );
}
