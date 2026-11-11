import { AudioLines, Camera, Clapperboard, Home, ListVideo, MonitorPlay, Scissors, Settings } from "lucide-react";
import type { ReactNode } from "react";
import appIcon from "../../assets/enrecorder-icon.svg";
import { cn } from "../../lib/format";
import { useNav, type PageId } from "../../store/nav";
import { isActive, useRecording } from "../../store/recording";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { Tip } from "../ui/primitives";

export const PAGES: { id: PageId; label: string; icon: ReactNode; phase?: number; section: "studio" | "library" }[] = [
  { id: "home", label: "Home", icon: <Home className="h-4 w-4" />, section: "studio" },
  { id: "video", label: "Video Recorder", icon: <MonitorPlay className="h-4 w-4" />, section: "studio" },
  { id: "audio", label: "Audio Recorder", icon: <AudioLines className="h-4 w-4" />, phase: 3, section: "studio" },
  { id: "screenshot", label: "Screenshot Studio", icon: <Camera className="h-4 w-4" />, phase: 4, section: "studio" },
  { id: "editor", label: "Media Editor", icon: <Scissors className="h-4 w-4" />, phase: 5, section: "studio" },
  { id: "recordings", label: "Recordings", icon: <Clapperboard className="h-4 w-4" />, section: "library" },
  { id: "exports", label: "Export Center", icon: <ListVideo className="h-4 w-4" />, section: "library" },
];

function NavRow({ icon, label, active, onClick, collapsed, soon, shortcut, live }: { icon: ReactNode; label: string; active?: boolean; onClick: () => void; collapsed: boolean; soon?: boolean; shortcut?: string; live?: boolean }) {
  const btn = (
    <button
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "group flex h-8 w-full items-center gap-2.5 rounded-md px-2 text-left text-[13px] transition-colors",
        active ? "bg-accent/15 text-fg" : "text-muted hover:bg-raised hover:text-fg",
        collapsed && "justify-center px-0",
      )}
    >
      <span className={cn("relative shrink-0", active && "text-accent")}>
        {icon}
        {live && <span className="absolute -right-1 -top-1 h-2 w-2 animate-pulse rounded-full bg-danger" />}
      </span>
      {!collapsed && <span className="min-w-0 flex-1 truncate">{label}</span>}
      {!collapsed && soon && <span className="rounded bg-raised px-1 text-[10px] text-faint group-hover:bg-panel">soon</span>}
    </button>
  );
  return collapsed ? (
    <Tip label={soon ? `${label} (coming soon)` : label} shortcut={shortcut} side="right">
      {btn}
    </Tip>
  ) : (
    btn
  );
}

function Section({ title, children, collapsed }: { title: string; children: ReactNode; collapsed: boolean }) {
  return (
    <div className="mt-4 first:mt-1">
      {!collapsed ? <div className="mb-1 flex h-6 items-center px-2 text-[11px] font-semibold uppercase tracking-wider text-faint">{title}</div> : <div className="mx-3 mb-2 h-px bg-line" />}
      <div className="space-y-0.5">{children}</div>
    </div>
  );
}

export function Sidebar() {
  const collapsed = useSettings((s) => s.settings.sidebarCollapsed);
  const page = useNav((s) => s.page);
  const go = useNav((s) => s.go);
  const recording = useRecording((s) => isActive(s.snap));
  const rows = (section: "studio" | "library") =>
    PAGES.filter((p) => p.section === section).map((p) => (
      <NavRow key={p.id} collapsed={collapsed} icon={p.icon} label={p.label} active={page === p.id} soon={!!p.phase} shortcut={`Ctrl+${PAGES.indexOf(p) + 1}`} live={p.id === "video" && recording} onClick={() => go(p.id)} />
    ));
  return (
    <aside className={cn("flex shrink-0 flex-col border-r border-line bg-panel transition-[width] duration-200", collapsed ? "w-[52px]" : "w-[220px]")}>
      <div className={cn("flex h-12 items-center gap-2 px-4", collapsed && "justify-center px-0")}>
        <img src={appIcon} alt="" className="h-6 w-6 shrink-0" draggable={false} />
        {!collapsed && <span className="text-[14px] font-semibold tracking-tight">ENRecorder</span>}
      </div>
      <nav className="min-h-0 flex-1 overflow-y-auto px-2 pb-4" aria-label="Main">
        <Section title="Studio" collapsed={collapsed}>
          {rows("studio")}
        </Section>
        <Section title="Library" collapsed={collapsed}>
          {rows("library")}
        </Section>
      </nav>
      <div className="border-t border-line p-2">
        <NavRow collapsed={collapsed} icon={<Settings className="h-4 w-4" />} label="Settings" shortcut="Ctrl+," onClick={() => useUi.getState().openDialog("settings")} />
      </div>
    </aside>
  );
}
