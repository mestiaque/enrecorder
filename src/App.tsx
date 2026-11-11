import { UpdateDialog } from "./components/update/UpdateDialog";
import { scheduleUpdateCheck } from "./store/update";
import * as Tooltip from "@radix-ui/react-tooltip";
import { QueryClientProvider } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Sidebar } from "./components/layout/Sidebar";
import { StatusBar } from "./components/layout/StatusBar";
import { Toolbar } from "./components/layout/Toolbar";
import { SettingsDialog } from "./components/settings/SettingsDialog";
import { CountdownOverlay } from "./components/recording/LivePanel";
import { ConfirmHost, PromptHost, Toaster } from "./components/ui/overlays";
import { api, EVENTS } from "./lib/api";
import { queryClient } from "./lib/queryClient";
import { AudioRecorderPage } from "./pages/AudioRecorder";
import { EditorPage } from "./pages/Editor";
import { ExportsPage } from "./pages/Exports";
import { HomePage } from "./pages/Home";
import { RecordingsPage } from "./pages/Recordings";
import { ScreenshotPage } from "./pages/Screenshot";
import { VideoRecorderPage } from "./pages/VideoRecorder";
import { PAGE_IDS, useNav, type PageId } from "./store/nav";
import { useSettings } from "./store/settings";
import { startRecording } from "./store/recorderForm";
import { useRecording } from "./store/recording";
import { useSchedule, useShotRequest } from "./store/schedule";
import { confirmDialog, useUi } from "./store/ui";

function hexToRgb(hex: string): string {
  const m = hex.replace("#", "").match(/^([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  return m ? `${parseInt(m[1], 16)} ${parseInt(m[2], 16)} ${parseInt(m[3], 16)}` : "99 102 241";
}

function useTheme() {
  const theme = useSettings((s) => s.settings.theme);
  const accent = useSettings((s) => s.settings.accent);
  const animations = useSettings((s) => s.settings.animations);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || theme === "glass" || (theme === "system" && mq.matches);
      document.documentElement.classList.toggle("dark", dark);
      document.documentElement.classList.toggle("light", !dark);
      document.documentElement.classList.toggle("glass", theme === "glass");
    };
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);
  useEffect(() => document.documentElement.style.setProperty("--accent", hexToRgb(accent)), [accent]);
  useEffect(() => void document.documentElement.classList.toggle("reduce-motion", !animations), [animations]);
}

/** Ctrl+, settings · Ctrl+B sidebar · Ctrl+1…7 pages. */
function useKeys() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      if (!ctrl) return;
      const ui = useUi.getState();
      if (e.key === ",") {
        e.preventDefault();
        ui.openDialog("settings");
        return;
      }
      if (ui.dialog || ui.confirmReq || ui.promptReq) return;
      if (e.key.toLowerCase() === "b") {
        e.preventDefault();
        const s = useSettings.getState();
        void s.set("sidebarCollapsed", !s.settings.sidebarCollapsed);
        return;
      }
      const n = Number(e.key);
      if (n >= 1 && n <= PAGE_IDS.length) {
        e.preventDefault();
        useNav.getState().go(PAGE_IDS[n - 1]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

const PAGES: Record<PageId, () => JSX.Element> = {
  home: HomePage,
  video: VideoRecorderPage,
  audio: AudioRecorderPage,
  screenshot: ScreenshotPage,
  editor: EditorPage,
  recordings: RecordingsPage,
  exports: ExportsPage,
};

export default function App() {
  const [ready, setReady] = useState(false);
  const page = useNav((s) => s.page);
  const snap = useRecording((s) => s.snap);
  useTheme();
  useKeys();
  const checkUpdates = useSettings((s) => s.settings.checkUpdates);
  useEffect(() => (ready ? scheduleUpdateCheck(checkUpdates) : undefined), [ready, checkUpdates]);

  useEffect(() => {
    void useSettings.getState().load().then(() => setReady(true));
    // The webview's own context menu (Reload, Inspect…) is not part of the app.
    const onContext = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (t.tagName !== "INPUT" && t.tagName !== "TEXTAREA" && !t.closest("pre")) e.preventDefault();
    };
    window.addEventListener("contextmenu", onContext);
    let offRec: (() => void) | undefined;
    void useRecording.getState().init().then((f) => (offRec = f));
    let offSchedule: (() => void) | undefined;
    void useSchedule.getState().init().then((f) => (offSchedule = f));
    // Screenshot shortcut: open the Screenshot Studio and take one with its current mode.
    const unShot = listen(EVENTS.screenshotRequest, () => {
      useNav.getState().go("screenshot");
      useShotRequest.setState((s) => ({ n: s.n + 1 }));
    });
    // Ctrl+Shift+R / tray → start with the current form.
    const unStart = listen(EVENTS.startRequest, () => void startRecording());
    // Quit while recording: ask first; quitting stops and saves the recording.
    const unQuit = listen(EVENTS.confirmQuit, async () => {
      const ok = await confirmDialog({ title: "Quit while recording?", message: "The recording is stopped and saved, then ENRecorder quits.", confirmLabel: "Stop, save and quit", danger: true });
      if (ok) void api.quitApp();
    });
    return () => {
      window.removeEventListener("contextmenu", onContext);
      offRec?.();
      offSchedule?.();
      void unShot.then((f) => f());
      void unStart.then((f) => f());
      void unQuit.then((f) => f());
    };
  }, []);

  if (!ready) return <div className="h-full bg-bg" />;
  const Page = PAGES[page];

  return (
    <QueryClientProvider client={queryClient}>
      <Tooltip.Provider>
        <div className="flex h-full">
          <Sidebar />
          <main className="flex min-w-0 flex-1 flex-col">
            <Toolbar />
            <div className="min-h-0 flex-1 overflow-y-auto">
              <div key={page} className="mx-auto max-w-[1400px] animate-fade-in px-6 py-6">
                <Page />
              </div>
            </div>
            <StatusBar />
          </main>
        </div>
        {snap?.state === "countdown" && <CountdownOverlay snap={snap} />}
        <SettingsDialog />
        <ConfirmHost />
        <PromptHost />
        <Toaster />
        <UpdateDialog />
      </Tooltip.Provider>
    </QueryClientProvider>
  );
}
