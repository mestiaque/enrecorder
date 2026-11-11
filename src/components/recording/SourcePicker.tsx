import { useQuery } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { AppWindow, Crop, Monitor, MonitorSmartphone, RefreshCw, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api, errorMessage, EVENTS } from "../../lib/api";
import { cn } from "../../lib/format";
import type { CaptureSource } from "../../lib/types";
import { useRecorderForm } from "../../store/recorderForm";
import { toast } from "../../store/ui";
import { EmptyState, Panel, Pending } from "../ui/blocks";
import { Button, IconButton, Segmented } from "../ui/primitives";

type Tab = "screen" | "window" | "region";

export const sourceKey = (s: CaptureSource) => (s.type === "monitor" || s.type === "window" ? `${s.type}:${s.id}` : s.type);

export function sourceLabel(s: CaptureSource): string {
  switch (s.type) {
    case "desktop":
      return `Entire desktop · ${s.width}×${s.height}`;
    case "monitor":
      return `${s.name} · ${s.width}×${s.height}`;
    case "window":
      return s.title;
    case "region":
      return `Area ${s.width}×${s.height} at ${s.x}, ${s.y}`;
    case "portal":
      return "Screen or window chosen in the sharing dialog";
  }
}

function Choice({ active, icon, title, sub, onClick }: { active: boolean; icon: React.ReactNode; title: string; sub?: string; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      className={cn("flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors", active ? "border-accent bg-accent/10" : "border-line hover:border-faint hover:bg-raised")}
    >
      <span className={cn("flex h-9 w-9 shrink-0 items-center justify-center rounded-md", active ? "bg-accent text-accent-fg" : "bg-raised text-muted")}>{icon}</span>
      <span className="min-w-0">
        <span className="block truncate text-[13px]">{title}</span>
        {sub && <span className="block truncate text-[11px] text-faint">{sub}</span>}
      </span>
    </button>
  );
}

export function SourcePicker() {
  const { source, setSource } = useRecorderForm();
  const [tab, setTab] = useState<Tab>(source?.type === "window" ? "window" : source?.type === "region" ? "region" : "screen");
  const [filter, setFilter] = useState("");
  const q = useQuery({ queryKey: ["capture-sources"], queryFn: api.listCaptureSources, staleTime: 5000 });
  const sources = q.data?.sources ?? [];
  const screens = sources.filter((s) => s.type === "desktop" || s.type === "monitor");
  const windows = useMemo(
    () => sources.filter((s): s is Extract<CaptureSource, { type: "window" }> => s.type === "window").filter((w) => `${w.title} ${w.app}`.toLowerCase().includes(filter.toLowerCase())),
    [sources, filter],
  );

  const portal = sources.find((s) => s.type === "portal");
  // Default: the primary monitor (or the desktop); on Wayland the sharing dialog.
  useEffect(() => {
    if (portal) {
      if (source?.type !== "portal") setSource(portal);
      return;
    }
    if (source || !screens.length) return;
    setSource(screens.find((s) => s.type === "monitor" && s.primary) ?? screens[0]);
  }, [screens, portal, source, setSource]);

  // The area overlay reports back through an event.
  useEffect(() => {
    const un = listen<CaptureSource | null>(EVENTS.regionSelected, (e) => {
      if (e.payload) {
        setSource(e.payload);
        setTab("region");
      }
    });
    return () => void un.then((f) => f());
  }, [setSource]);

  const selectArea = () => api.openRegionSelector().catch((e) => toast({ kind: "error", title: "Could not open the area selector", detail: errorMessage(e) }));

  return (
    <Panel
      title="What to record"
      actions={
        <IconButton label="Refresh the list" onClick={() => void q.refetch()}>
          <RefreshCw className={cn("h-3.5 w-3.5", q.isFetching && "animate-spin")} />
        </IconButton>
      }
    >
      {q.isLoading ? (
        <Pending label="Looking for screens and windows…" />
      ) : q.isError ? (
        <div className="text-[13px] text-danger">{errorMessage(q.error)}</div>
      ) : q.data?.unsupported ? (
        <EmptyState compact icon={<MonitorSmartphone className="h-5 w-5" />} title="Recording is not available here yet" text={q.data.unsupported} />
      ) : portal ? (
        <div className="space-y-2">
          <Choice active icon={<MonitorSmartphone className="h-4 w-4" />} title="Choose in the sharing dialog" sub="Wayland: your desktop asks which screen or window to share when you press Start" onClick={() => setSource(portal)} />
          <div className="text-[11px] text-faint">ENRecorder never sees the screen without your choice in that dialog. Cancelling it cancels the recording.</div>
        </div>
      ) : (
        <>
          <Segmented
            value={tab}
            onChange={setTab}
            options={[
              { value: "screen", label: <><Monitor className="h-3.5 w-3.5" />Screen</> },
              { value: "window", label: <><AppWindow className="h-3.5 w-3.5" />Window</> },
              { value: "region", label: <><Crop className="h-3.5 w-3.5" />Area</> },
            ]}
          />
          <div className="mt-3 space-y-2">
            {tab === "screen" &&
              screens.map((s) => (
                <Choice
                  key={sourceKey(s)}
                  active={!!source && sourceKey(source) === sourceKey(s)}
                  icon={<Monitor className="h-4 w-4" />}
                  title={s.type === "desktop" ? "Entire desktop" : `${s.name}${s.primary ? " (primary)" : ""}`}
                  sub={`${s.width}×${s.height}${s.type === "monitor" ? ` · at ${s.x}, ${s.y}` : screens.length > 2 ? " · all monitors" : ""}`}
                  onClick={() => setSource(s)}
                />
              ))}
            {tab === "window" && (
              <>
                <div className="relative">
                  <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-faint" />
                  <input
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                    placeholder="Find a window"
                    aria-label="Find a window"
                    className="h-8 w-full rounded-md border border-line bg-bg pl-8 pr-2 text-[13px] outline-none placeholder:text-faint focus:border-accent"
                  />
                </div>
                <div className="max-h-[320px] space-y-2 overflow-y-auto pr-1">
                  {windows.length === 0 && <div className="py-4 text-center text-[12px] text-faint">No windows found</div>}
                  {windows.map((w) => (
                    <Choice key={w.id} active={!!source && sourceKey(source) === sourceKey(w)} icon={<AppWindow className="h-4 w-4" />} title={w.title} sub={`${w.app} · ${w.width}×${w.height}`} onClick={() => setSource(w)} />
                  ))}
                </div>
                {q.data?.backend === "x11" && <div className="text-[11px] text-faint">On X11 a window is recorded as it appears on screen: anything covering it is recorded too, and it keeps the size it had when recording started.</div>}
              </>
            )}
            {tab === "region" && (
              <div className="space-y-2">
                {source?.type === "region" && <Choice active icon={<Crop className="h-4 w-4" />} title={`${source.width}×${source.height}`} sub={`at ${source.x}, ${source.y}`} onClick={() => void selectArea()} />}
                <Button variant={source?.type === "region" ? "default" : "primary"} onClick={() => void selectArea()}>
                  <Crop className="h-3.5 w-3.5" />
                  {source?.type === "region" ? "Select another area…" : "Select an area…"}
                </Button>
                <div className="text-[11px] text-faint">Drag over the screen to mark the area, then confirm. Esc cancels.</div>
              </div>
            )}
          </div>
        </>
      )}
    </Panel>
  );
}
