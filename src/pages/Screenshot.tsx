import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AppWindow, Camera, Crop, FolderOpen, Image as ImageIcon, Monitor, Pencil } from "lucide-react";
import { useEffect, useState } from "react";
import { ScreenshotEditor } from "../components/screenshot/Editor";
import type { Tool } from "../components/screenshot/annotate";
import { EmptyState, PageHeader, Panel, Row } from "../components/ui/blocks";
import { Button, Select, Switch } from "../components/ui/primitives";
import { api, errorMessage } from "../lib/api";
import { formatBytes } from "../lib/format";
import { useMedia, usePaths } from "../lib/queries";
import type { CaptureSource, Screenshot } from "../lib/types";
import { toast } from "../store/ui";
import { useShotRequest } from "../store/schedule";

type Mode = "screen" | "monitor" | "window" | "area";

function Mode({ active, icon, label, hint, onClick }: { active: boolean; icon: React.ReactNode; label: string; hint: string; onClick: () => void }) {
  return (
    <button onClick={onClick} className={`flex flex-col items-start gap-1 rounded-lg border p-3 text-left transition-colors ${active ? "border-accent bg-accent/10" : "border-line hover:border-faint"}`}>
      <span className={active ? "text-accent" : "text-muted"}>{icon}</span>
      <span className="text-[13px] font-medium">{label}</span>
      <span className="text-[11px] text-faint">{hint}</span>
    </button>
  );
}

export function ScreenshotPage() {
  const qc = useQueryClient();
  const { data: paths } = usePaths();
  const sources = useQuery({ queryKey: ["capture-sources"], queryFn: api.listCaptureSources, staleTime: 5000 });
  const recent = useMedia({ filter: "screenshot", search: "", sort: "newest", limit: 24 });
  const [mode, setMode] = useState<Mode>("screen");
  const [delay, setDelay] = useState(0);
  const [hide, setHide] = useState(true);
  const [cursor, setCursor] = useState(false);
  const [pick, setPick] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<{ shot: Screenshot; tool: Tool } | null>(null);

  const portal = sources.data?.backend === "waylandPortal";
  const list = sources.data?.sources ?? [];
  const monitors = list.filter((s): s is Extract<CaptureSource, { type: "monitor" }> => s.type === "monitor");
  const windows = list.filter((s): s is Extract<CaptureSource, { type: "window" }> => s.type === "window");
  const desktop = list.find((s) => s.type === "desktop") ?? null;

  const source = (): CaptureSource | null => {
    if (portal) return null;
    if (mode === "monitor") return monitors.find((m) => m.id === pick) ?? monitors[0] ?? null;
    if (mode === "window") return windows.find((w) => w.id === pick) ?? null;
    return desktop;
  };

  const capture = async () => {
    const src = source();
    if (!portal && !src) return toast({ kind: "info", title: mode === "window" ? "Choose a window first" : "Nothing to capture" });
    setBusy(true);
    try {
      const shot = await api.takeScreenshot(src, delay, hide, cursor);
      void qc.invalidateQueries({ queryKey: ["media"] });
      void qc.invalidateQueries({ queryKey: ["dashboard"] });
      setEditing({ shot, tool: mode === "area" ? "crop" : "arrow" });
    } catch (e) {
      toast({ kind: "error", title: "Could not take the screenshot", detail: errorMessage(e) }, 8000);
    } finally {
      setBusy(false);
    }
  };

  // Screenshot shortcut (Settings → Shortcuts): take one as soon as the sources are known.
  const request = useShotRequest((st) => st.n);
  const [handled, setHandled] = useState(request);
  useEffect(() => {
    if (request !== handled && !busy && (sources.data || portal)) {
      setHandled(request);
      void capture();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request, sources.data]);

  const open = async (id: string) => {
    try {
      setEditing({ shot: await api.getScreenshot(id), tool: "select" });
    } catch (e) {
      toast({ kind: "error", title: "Could not open", detail: errorMessage(e) });
    }
  };

  return (
    <>
      <PageHeader title="Screenshot Studio" subtitle="Capture the screen, a monitor, a window or an area, then annotate and export." />
      <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
        <div className="min-w-0 space-y-5">
          <Panel title="Capture">
            {portal ? (
              <p className="pb-2 text-[12px] text-muted">On Wayland your desktop shows its own screenshot dialog, where you choose the screen, a window or an area.</p>
            ) : (
              <div className="grid grid-cols-2 gap-2 pb-3 sm:grid-cols-4">
                <Mode active={mode === "screen"} icon={<Monitor className="h-5 w-5" />} label="Full screen" hint="All monitors" onClick={() => setMode("screen")} />
                <Mode active={mode === "monitor"} icon={<ImageIcon className="h-5 w-5" />} label="Monitor" hint={`${monitors.length} connected`} onClick={() => setMode("monitor")} />
                <Mode active={mode === "window"} icon={<AppWindow className="h-5 w-5" />} label="Window" hint="One app window" onClick={() => setMode("window")} />
                <Mode active={mode === "area"} icon={<Crop className="h-5 w-5" />} label="Area" hint="Crop after capture" onClick={() => setMode("area")} />
              </div>
            )}
            {!portal && mode === "monitor" && monitors.length > 1 && (
              <Row label="Monitor">
                <Select value={pick || monitors[0]?.id || ""} onChange={setPick} options={monitors.map((m) => ({ value: m.id, label: `${m.name} · ${m.width}×${m.height}` }))} />
              </Row>
            )}
            {!portal && mode === "window" && (
              <Row label="Window">
                <Select value={pick} onChange={setPick} className="max-w-[320px]" options={[{ value: "", label: windows.length ? "Choose a window…" : "No windows found" }, ...windows.map((w) => ({ value: w.id, label: `${w.title}${w.app ? ` — ${w.app}` : ""}` }))]} />
              </Row>
            )}
            <Row label="Delay" hint="Time to open a menu or move the pointer">
              <Select value={delay} onChange={setDelay} options={[0, 3, 5, 10].map((n) => ({ value: n, label: n ? `${n} seconds` : "None" }))} />
            </Row>
            <Row label="Hide ENRecorder while capturing">
              <Switch checked={hide} onChange={setHide} label="Hide ENRecorder while capturing" />
            </Row>
            {!portal && (
              <Row label="Include the mouse pointer">
                <Switch checked={cursor} onChange={setCursor} label="Include the mouse pointer" />
              </Row>
            )}
            <div className="flex items-center gap-3 pt-3">
              <Button variant="primary" className="h-11 px-5 text-[14px]" disabled={busy || sources.isLoading} onClick={() => void capture()}>
                <Camera className="h-4 w-4" />
                {busy ? (delay ? `Capturing in ${delay} s…` : "Capturing…") : "Take screenshot"}
              </Button>
              {sources.data?.unsupported && <span className="text-[12px] text-danger">{sources.data.unsupported}</span>}
            </div>
          </Panel>
        </div>
        <Panel
          title="Recent screenshots"
          actions={
            <Button size="sm" variant="ghost" disabled={!paths} onClick={() => paths && api.openFolder(paths.screenshots).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) }))}>
              <FolderOpen className="h-3.5 w-3.5" />
            </Button>
          }
          bodyClass="py-1"
        >
          {!recent.data?.length ? (
            <EmptyState compact icon={<Camera className="h-6 w-6" />} title="No screenshots yet" text="They appear here and in Recordings." />
          ) : (
            recent.data.map((m) => (
              <Row key={m.id} label={<span className="block max-w-[200px] truncate">{m.title}</span>} hint={`${m.width}×${m.height} · ${formatBytes(m.sizeBytes)}`}>
                <Button size="sm" onClick={() => void open(m.id)}>
                  <Pencil className="h-3.5 w-3.5" />
                  Edit
                </Button>
              </Row>
            ))
          )}
        </Panel>
      </div>
      {editing && (
        <ScreenshotEditor
          shot={editing.shot}
          startTool={editing.tool}
          onClose={() => {
            setEditing(null);
            void qc.invalidateQueries({ queryKey: ["media"] });
          }}
        />
      )}
    </>
  );
}
