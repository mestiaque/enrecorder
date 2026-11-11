import { Copy, EyeOff, FolderOpen, Monitor, Moon, MoreHorizontal, PanelLeft, Power, Settings, Sparkles, Sun } from "lucide-react";
import { api, errorMessage } from "../../lib/api";
import { usePaths } from "../../lib/queries";
import { useNav } from "../../store/nav";
import { useSettings } from "../../store/settings";
import { toast, useUi } from "../../store/ui";
import { DropdownMenu } from "../ui/menus";
import { IconButton } from "../ui/primitives";
import { WindowControls, windowDragProps } from "../ui/WindowControls";
import { PAGES } from "./Sidebar";

async function copyDiagnostics() {
  try {
    await navigator.clipboard.writeText(await api.diagnostics());
    toast({ kind: "success", title: "Diagnostics copied", detail: "Paste them into a bug report." });
  } catch (e) {
    toast({ kind: "error", title: "Could not copy diagnostics", detail: errorMessage(e) });
  }
}

export function Toolbar() {
  const { settings, set } = useSettings();
  const page = useNav((s) => s.page);
  const { data: paths } = usePaths();
  const title = PAGES.find((p) => p.id === page)?.label ?? "";
  const themeIcon =
    settings.theme === "light" ? <Sun className="h-4 w-4" /> : settings.theme === "dark" ? <Moon className="h-4 w-4" /> : settings.theme === "glass" ? <Sparkles className="h-4 w-4" /> : <Monitor className="h-4 w-4" />;
  const openFolder = (p?: string) => p && api.openFolder(p).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) }));

  return (
    <header className="flex h-12 shrink-0 items-center gap-1 border-b border-line bg-panel/80 px-2 backdrop-blur" {...windowDragProps}>
      <IconButton label={settings.sidebarCollapsed ? "Show sidebar" : "Hide sidebar"} shortcut="Ctrl+B" onClick={() => void set("sidebarCollapsed", !settings.sidebarCollapsed)}>
        <PanelLeft className="h-4 w-4" />
      </IconButton>
      <div className="ml-1 min-w-0 flex-1 truncate text-[13px] font-medium">{title}</div>
      <DropdownMenu
        trigger={<IconButton label="Theme">{themeIcon}</IconButton>}
        entries={[
          { label: "Dark", icon: <Moon className="h-3.5 w-3.5" />, checked: settings.theme === "dark", onSelect: () => void set("theme", "dark") },
          { label: "Light", icon: <Sun className="h-3.5 w-3.5" />, checked: settings.theme === "light", onSelect: () => void set("theme", "light") },
          { label: "Glass", icon: <Sparkles className="h-3.5 w-3.5" />, checked: settings.theme === "glass", onSelect: () => void set("theme", "glass") },
          { label: "System", icon: <Monitor className="h-3.5 w-3.5" />, checked: settings.theme === "system", onSelect: () => void set("theme", "system") },
        ]}
      />
      <DropdownMenu
        trigger={
          <IconButton label="More">
            <MoreHorizontal className="h-4 w-4" />
          </IconButton>
        }
        entries={[
          { label: "Open recordings folder", icon: <FolderOpen className="h-3.5 w-3.5" />, disabled: !paths, onSelect: () => void openFolder(paths?.recordings) },
          { label: "Open screenshots folder", icon: <FolderOpen className="h-3.5 w-3.5" />, disabled: !paths, onSelect: () => void openFolder(paths?.screenshots) },
          { separator: true },
          { label: "Copy diagnostics", icon: <Copy className="h-3.5 w-3.5" />, onSelect: () => void copyDiagnostics() },
          { separator: true },
          { label: "Hide to tray", icon: <EyeOff className="h-3.5 w-3.5" />, onSelect: () => void api.hideToTray() },
          { label: "Quit ENRecorder", icon: <Power className="h-3.5 w-3.5" />, danger: true, onSelect: () => void api.quitApp() },
        ]}
      />
      <IconButton label="Settings" shortcut="Ctrl+," onClick={() => useUi.getState().openDialog("settings")}>
        <Settings className="h-4 w-4" />
      </IconButton>
      <div className="mx-1 h-5 w-px bg-line" />
      <WindowControls />
    </header>
  );
}
