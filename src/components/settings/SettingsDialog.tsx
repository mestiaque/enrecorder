import { getVersion } from "@tauri-apps/api/app";
import { useUpdate } from "../../store/update";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useQueryClient } from "@tanstack/react-query";
import { Check, Copy, FolderOpen, RefreshCw } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, errorMessage } from "../../lib/api";
import { cn } from "../../lib/format";
import { keys, useTools } from "../../lib/queries";
import { shortVersion } from "../../lib/readiness";
import type { Settings } from "../../lib/types";
import { useSettings } from "../../store/settings";
import { confirmDialog, toast, useUi } from "../../store/ui";
import { EncoderTable } from "../media/EncoderTable";
import { Dialog } from "../ui/overlays";
import { Button, Switch } from "../ui/primitives";
import { AboutSection } from "./AboutSection";
import { DevicesSection, PerformanceSection, PrivacySection, ScreenshotSection, ShortcutsSection, TranscriptionSection } from "./MoreSections";

const SECTIONS = [
  ["general", "General"],
  ["storage", "Storage"],
  ["tools", "Tools"],
  ["appearance", "Appearance"],
  ["screenshots", "Screenshots"],
  ["shortcuts", "Shortcuts"],
  ["devices", "Devices"],
  ["performance", "Performance"],
  ["privacy", "Privacy"],
  ["logs", "Logs"],
  ["about", "About"],
] as const;

const THEME_CARDS: { id: Settings["theme"]; label: string; bg: string; side: string; bar: string; tile: string }[] = [
  { id: "dark", label: "Dark", bg: "#131315", side: "#19191c", bar: "#2c2c32", tile: "#222226" },
  { id: "light", label: "Light", bg: "#f4f4f6", side: "#fcfcfd", bar: "#dcdce2", tile: "#ffffff" },
  {
    id: "glass",
    label: "Glass",
    bg: "radial-gradient(circle at 15% 20%, rgba(139,108,255,.9), transparent 55%), radial-gradient(circle at 90% 10%, rgba(226,64,122,.8), transparent 50%), radial-gradient(circle at 70% 95%, rgba(56,189,248,.75), transparent 55%), #1a1530",
    side: "rgba(255,255,255,.14)",
    bar: "rgba(255,255,255,.3)",
    tile: "rgba(255,255,255,.18)",
  },
  { id: "system", label: "System", bg: "linear-gradient(135deg, #f4f4f6 50%, #131315 50%)", side: "rgba(127,127,127,.25)", bar: "rgba(127,127,127,.4)", tile: "rgba(127,127,127,.3)" },
];

/** Theme picker with small previews (same design as the other EN* apps). */
function ThemeCards({ value, onChange }: { value: Settings["theme"]; onChange: (v: Settings["theme"]) => void }) {
  return (
    <div className="border-b border-line/60 py-3">
      <div className="text-[13px]">Theme</div>
      <div className="mt-2.5 grid grid-cols-4 gap-3">
        {THEME_CARDS.map((t) => (
          <button
            key={t.id}
            onClick={() => onChange(t.id)}
            className={cn("rounded-[10px] border p-2 pb-1.5 text-[12px] transition-colors", value === t.id ? "border-accent bg-accent/[0.07] text-fg ring-1 ring-accent" : "border-line text-muted hover:border-faint")}
          >
            <div className="flex h-[70px] overflow-hidden rounded-md border border-black/10" style={{ background: t.bg }}>
              <div className="w-[26%]" style={{ background: t.side }} />
              <div className="flex flex-1 flex-col gap-1.5 p-1.5">
                <div className="h-2 rounded-sm" style={{ background: t.bar }} />
                <div className="flex flex-1 flex-col gap-1">
                  {[0, 1, 2].map((i) => (
                    <div key={i} className="flex-1 rounded-[3px]" style={{ background: t.tile }} />
                  ))}
                </div>
              </div>
            </div>
            <div className="mt-1.5">{t.label}</div>
          </button>
        ))}
      </div>
      <div className="mt-2 text-[11px] text-faint">System follows your desktop's light or dark setting.</div>
    </div>
  );
}

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

function Toggle({ k, label, hint }: { k: BoolKey; label: string; hint?: string }) {
  const { settings, set } = useSettings();
  return (
    <Field label={label} hint={hint}>
      <Switch checked={settings[k]} onChange={(v) => void set(k, v)} label={label} />
    </Field>
  );
}

const ACCENTS = ["#6366f1", "#3b82f6", "#8b5cf6", "#ec4899", "#ef4444", "#f97316", "#eab308", "#22c55e", "#14b8a6", "#06b6d4"];

function UpdateRow() {
  const check = useUpdate((u) => u.check);
  const checking = useUpdate((u) => u.checking);
  const [status, setStatus] = useState("");
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion().then(setVersion, () => {});
  }, []);
  return (
    <Field label="Updates" hint={status || `You have version ${version}`}>
      <Button size="sm" disabled={checking} onClick={async () => setStatus(await check(true))}>
        {checking ? "Checking…" : "Check now"}
      </Button>
    </Field>
  );
}

function GeneralSection() {
  const apply = useSettings((s) => s.apply);
  return (
    <>
      <Toggle k="startWithOs" label="Start with the system" hint="ENRecorder starts in the tray when you log in" />
      <Toggle k="startMinimized" label="Start minimized to the tray" />
      <Toggle k="closeToTray" label="Keep running in the tray when the window is closed" hint="Quit from the tray menu or More → Quit" />
      <Toggle k="checkUpdates" label="Check for updates automatically" hint="Looks for a new ENRecorder release on GitHub once a day" />
      <UpdateRow />
      <Field label="Reset all settings" hint="Your recordings and screenshots are not touched">
        <Button
          size="sm"
          variant="danger"
          onClick={async () => {
            if (!(await confirmDialog({ title: "Reset all settings?", message: "Every setting goes back to its default. Recordings, screenshots and the library are kept.", confirmLabel: "Reset", danger: true }))) return;
            try {
              apply(await api.resetSettings());
              toast({ kind: "success", title: "Settings reset" });
            } catch (e) {
              toast({ kind: "error", title: "Could not reset settings", detail: errorMessage(e) });
            }
          }}
        >
          Reset
        </Button>
      </Field>
    </>
  );
}

function FolderField({ k, label, fallback }: { k: "recordingsDir" | "screenshotsDir"; label: string; fallback: string }) {
  const { settings, set } = useSettings();
  const qc = useQueryClient();
  const value = settings[k];
  const save = async (v: string) => {
    if (await set(k, v)) {
      void qc.invalidateQueries({ queryKey: keys.paths });
      void qc.invalidateQueries({ queryKey: keys.dashboard });
    }
  };
  return (
    <Field label={label} hint={<span className="font-mono">{value || `${fallback} (default)`}</span>}>
      <div className="flex gap-2">
        {value && (
          <Button size="sm" variant="ghost" onClick={() => void save("")}>
            Use default
          </Button>
        )}
        <Button
          size="sm"
          onClick={async () => {
            const d = await openDialog({ directory: true, title: label, defaultPath: value || undefined });
            if (typeof d === "string") await save(d);
          }}
        >
          <FolderOpen className="h-3.5 w-3.5" />
          Choose…
        </Button>
      </div>
    </Field>
  );
}

function StorageSection() {
  const { defaultRecordings, defaultScreenshots } = useSettings();
  return (
    <>
      <div className="pt-3 text-[11px] text-faint">A chosen folder is checked before it is saved: it must be a full path ENRecorder can write to.</div>
      <FolderField k="recordingsDir" label="Recordings folder" fallback={defaultRecordings} />
      <FolderField k="screenshotsDir" label="Screenshots folder" fallback={defaultScreenshots} />
    </>
  );
}

function ToolsSection() {
  const { settings, set } = useSettings();
  const { data: tools } = useTools();
  return (
    <div className="space-y-4 py-3">
      <div>
        <Field
          label={tools?.ffmpeg ? `FFmpeg ${shortVersion(tools.ffmpeg.version)}` : "FFmpeg"}
          hint={
            tools?.ffmpeg ? (
              <span className="font-mono">{tools.ffmpeg.path}</span>
            ) : tools?.checking ? (
              "Looking for FFmpeg…"
            ) : (
              <span className="text-danger">Not found. Ubuntu: sudo apt install ffmpeg — or choose the program below.</span>
            )
          }
        >
          <span className={cn("text-[12px]", tools?.ffmpeg ? "text-success" : "text-faint")}>{tools?.ffmpeg ? "Found" : "—"}</span>
        </Field>
        <Field label={tools?.ffprobe ? `FFprobe ${shortVersion(tools.ffprobe.version)}` : "FFprobe"} hint={tools?.ffprobe ? <span className="font-mono">{tools.ffprobe.path}</span> : "Reads media details for the library"}>
          <span className={cn("text-[12px]", tools?.ffprobe ? "text-success" : "text-faint")}>{tools?.ffprobe ? "Found" : "—"}</span>
        </Field>
        <Field label="Use a specific FFmpeg" hint={settings.ffmpegPath ? <span className="font-mono">{settings.ffmpegPath}</span> : "Found automatically (bundled copy, then PATH)"}>
          <div className="flex gap-2">
            {settings.ffmpegPath && (
              <Button size="sm" variant="ghost" onClick={() => void set("ffmpegPath", "")}>
                Automatic
              </Button>
            )}
            <Button
              size="sm"
              onClick={async () => {
                const p = await openDialog({ title: "Choose the FFmpeg program", multiple: false, directory: false });
                if (typeof p === "string") void set("ffmpegPath", p);
              }}
            >
              Choose…
            </Button>
          </div>
        </Field>
      </div>
      <EncoderTable />
      <TranscriptionSection />
    </div>
  );
}

function AppearanceSection() {
  const { settings: s, set } = useSettings();
  return (
    <>
      <ThemeCards value={s.theme} onChange={(v) => void set("theme", v)} />
      <Field label="Accent color">
        <div className="flex gap-1.5">
          {ACCENTS.map((c) => (
            <button key={c} onClick={() => void set("accent", c)} className="flex h-6 w-6 items-center justify-center rounded-full" style={{ background: c, boxShadow: s.accent === c ? `0 0 0 2px rgb(var(--panel)), 0 0 0 4px ${c}` : undefined }} aria-label={c}>
              {s.accent === c && <Check className="h-3.5 w-3.5 text-white" />}
            </button>
          ))}
        </div>
      </Field>
      <Toggle k="animations" label="Animations" hint="Turn off to reduce motion" />
    </>
  );
}

function LogsSection() {
  const [text, setText] = useState("");
  const ref = useRef<HTMLPreElement>(null);
  const load = async () => {
    setText(await api.readLogs(400).catch(() => ""));
    requestAnimationFrame(() => ref.current && (ref.current.scrollTop = ref.current.scrollHeight));
  };
  useEffect(() => {
    void load();
  }, []);
  return (
    <div className="flex h-full flex-col gap-3 py-3">
      <div className="flex items-center gap-2">
        <div className="flex-1 text-[11px] text-faint">Logs stay on this computer (5 MB, then the oldest part is dropped). Nothing is uploaded.</div>
        <Button size="sm" variant="ghost" onClick={load}>
          <RefreshCw className="h-3.5 w-3.5" />
          Refresh
        </Button>
        <Button size="sm" onClick={() => void api.openLogFolder()}>
          <FolderOpen className="h-3.5 w-3.5" />
          Open folder
        </Button>
        <Button
          size="sm"
          variant="primary"
          onClick={async () => {
            try {
              await navigator.clipboard.writeText(await api.diagnostics());
              toast({ kind: "success", title: "Diagnostics copied", detail: "Paste them into a bug report." });
            } catch (e) {
              toast({ kind: "error", title: "Could not copy diagnostics", detail: errorMessage(e) });
            }
          }}
        >
          <Copy className="h-3.5 w-3.5" />
          Copy diagnostics
        </Button>
      </div>
      <pre ref={ref} className="min-h-[300px] flex-1 select-text overflow-auto whitespace-pre-wrap break-all rounded-lg border border-line bg-bg p-3 font-mono text-[11px] leading-relaxed text-muted">
        {text || "The log is empty."}
      </pre>
    </div>
  );
}

export function SettingsDialog() {
  const open = useUi((s) => s.dialog === "settings");
  const section = useUi((s) => s.settingsSection);
  const current = SECTIONS.some(([id]) => id === section) ? section : "general";
  return (
    <Dialog open={open} onOpenChange={(o) => !o && useUi.getState().openDialog(null)} title="Settings" className="h-[min(660px,88vh)] w-[min(880px,94vw)]">
      <div className="-mx-5 -my-4 flex h-full min-h-[480px]">
        <nav className="w-44 shrink-0 space-y-0.5 border-r border-line p-2">
          {SECTIONS.map(([id, label]) => (
            <button
              key={id}
              onClick={() => useUi.setState({ settingsSection: id })}
              className={cn("block h-8 w-full rounded-md px-3 text-left text-[13px]", current === id ? "bg-accent/15 text-fg" : "text-muted hover:bg-raised hover:text-fg")}
            >
              {label}
            </button>
          ))}
        </nav>
        <div className="min-w-0 flex-1 overflow-y-auto px-6 py-2">
          {current === "general" && <GeneralSection />}
          {current === "storage" && <StorageSection />}
          {current === "tools" && <ToolsSection />}
          {current === "appearance" && <AppearanceSection />}
          {current === "screenshots" && <ScreenshotSection />}
          {current === "shortcuts" && <ShortcutsSection />}
          {current === "devices" && <DevicesSection />}
          {current === "performance" && <PerformanceSection />}
          {current === "privacy" && <PrivacySection />}
          {current === "logs" && <LogsSection />}
          {current === "about" && <AboutSection />}
        </div>
      </div>
    </Dialog>
  );
}
