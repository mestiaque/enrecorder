import { AudioLines, Camera, Clapperboard, FolderOpen, HardDrive, Image as ImageIcon, MonitorPlay, RefreshCw } from "lucide-react";
import type { ReactNode } from "react";
import { EmptyState, PageHeader, Panel, StatusIcon } from "../components/ui/blocks";
import { MediaRow } from "../components/media/MediaList";
import { Button, IconButton, ProgressBar } from "../components/ui/primitives";
import { api, errorMessage } from "../lib/api";
import { formatBytes, formatTotalTime } from "../lib/format";
import { useDashboard, useSystem, useTools } from "../lib/queries";
import { readinessChecks } from "../lib/readiness";
import { useNav, type PageId } from "../store/nav";
import { toast } from "../store/ui";

function QuickAction({ icon, title, text, phase, page }: { icon: ReactNode; title: string; text: string; phase?: number; page: PageId }) {
  return (
    <button
      onClick={() => useNav.getState().go(page)}
      className="group flex items-start gap-3 rounded-xl border border-line bg-panel p-4 text-left transition-colors hover:border-accent/50 hover:bg-raised"
    >
      <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-accent/15 text-accent">{icon}</span>
      <span className="min-w-0">
        <span className="flex items-center gap-2 text-[14px] font-medium">
          {title}
          {phase && <span className="rounded bg-raised px-1.5 py-px text-[10px] font-semibold text-faint group-hover:bg-panel">Phase {phase}</span>}
        </span>
        <span className="mt-0.5 block text-[12px] text-muted">{text}</span>
      </span>
    </button>
  );
}

function Stat({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="text-[11px] uppercase tracking-wider text-faint">{label}</div>
      <div className="mt-0.5 truncate text-[18px] font-semibold tabular-nums">{value}</div>
    </div>
  );
}

const openFolder = (path: string) => api.openFolder(path).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) }));

export function HomePage() {
  const dash = useDashboard();
  const sys = useSystem();
  const tools = useTools();
  const d = dash.data;
  const checks = readinessChecks(sys.data, tools.data, d?.disk);
  const used = d ? d.recordings.bytes + d.screenshots.bytes : 0;

  return (
    <>
      <PageHeader title="Home" subtitle="Record your screen and audio, take screenshots, then trim and export them." />

      <div className="grid gap-3 md:grid-cols-3">
        <QuickAction page="video" icon={<MonitorPlay className="h-5 w-5" />} title="Record the screen" text="The desktop, a monitor, a window or an area. Ctrl+Shift+R starts and stops." />
        <QuickAction page="audio" phase={3} icon={<AudioLines className="h-5 w-5" />} title="Audio Recorder" text="Microphone and system audio on separate tracks." />
        <QuickAction page="screenshot" phase={4} icon={<Camera className="h-5 w-5" />} title="Screenshot Studio" text="Full screen, window or region, then annotate." />
      </div>

      <div className="mt-5 grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
        <div className="min-w-0 space-y-5">
          <Panel title="Recent recordings" bodyClass="p-2">
            {d?.recentRecordings.length ? (
              d.recentRecordings.map((m) => <MediaRow key={m.id} item={m} />)
            ) : (
              <EmptyState compact icon={<Clapperboard className="h-5 w-5" />} title={dash.isLoading ? "Loading…" : "No recordings yet"} text="Videos and audio you record appear here." />
            )}
          </Panel>
          <Panel title="Recent screenshots" bodyClass="p-2">
            {d?.recentScreenshots.length ? (
              d.recentScreenshots.map((m) => <MediaRow key={m.id} item={m} />)
            ) : (
              <EmptyState compact icon={<ImageIcon className="h-5 w-5" />} title={dash.isLoading ? "Loading…" : "No screenshots yet"} text="Screenshots you take appear here." />
            )}
          </Panel>
          <Panel title="Statistics">
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-5">
              <Stat label="Videos" value={d?.stats.videos ?? "—"} />
              <Stat label="Audio" value={d?.stats.audio ?? "—"} />
              <Stat label="Screenshots" value={d?.stats.screenshots ?? "—"} />
              <Stat label="Recorded" value={d ? formatTotalTime(d.stats.totalDurationMs) : "—"} />
              <Stat label="Last 7 days" value={d?.stats.last7Days ?? "—"} />
            </div>
          </Panel>
        </div>

        <div className="min-w-0 space-y-5">
          <Panel
            title="System readiness"
            actions={
              <IconButton
                label="Check again"
                onClick={() => {
                  void sys.refetch();
                  void dash.refetch();
                }}
              >
                <RefreshCw className="h-3.5 w-3.5" />
              </IconButton>
            }
            bodyClass="py-1"
          >
            {checks.map((c) => (
              <div key={c.id} className="flex items-start gap-2.5 border-b border-line/60 py-2.5 last:border-0">
                <StatusIcon level={c.level} className="mt-0.5" />
                <div className="min-w-0">
                  <div className="text-[13px]">{c.label}</div>
                  <div className="text-[11px] text-faint">{c.detail}</div>
                </div>
              </div>
            ))}
          </Panel>

          <Panel title="Storage">
            {d ? (
              <div className="space-y-3 text-[13px]">
                {d.disk && (
                  <div>
                    <div className="mb-1.5 flex items-center justify-between">
                      <span className="flex items-center gap-1.5">
                        <HardDrive className="h-3.5 w-3.5 text-faint" />
                        Drive
                      </span>
                      <span className="tabular-nums text-muted">
                        {formatBytes(d.disk.available)} free of {formatBytes(d.disk.total)}
                      </span>
                    </div>
                    <ProgressBar value={d.disk.total - d.disk.available} total={d.disk.total} />
                  </div>
                )}
                {[
                  ["Recordings", d.recordings],
                  ["Screenshots", d.screenshots],
                ].map(([label, u]) => {
                  const usage = u as typeof d.recordings;
                  return (
                    <div key={label as string} className="flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div>{label as string}</div>
                        <div className="truncate text-[11px] text-faint" title={usage.path}>
                          {usage.path}
                        </div>
                      </div>
                      <div className="flex shrink-0 items-center gap-1">
                        <span className="tabular-nums text-muted">{usage.exists ? `${formatBytes(usage.bytes)}${usage.truncated ? "+" : ""}` : "empty"}</span>
                        <IconButton label="Open folder" onClick={() => void openFolder(usage.path)}>
                          <FolderOpen className="h-3.5 w-3.5" />
                        </IconButton>
                      </div>
                    </div>
                  );
                })}
                <div className="border-t border-line/60 pt-2 text-[11px] text-faint">ENRecorder uses {formatBytes(used)} in its folders.</div>
              </div>
            ) : dash.isError ? (
              <div className="text-[13px] text-danger">
                {errorMessage(dash.error)}{" "}
                <Button size="sm" variant="ghost" onClick={() => void dash.refetch()}>
                  Try again
                </Button>
              </div>
            ) : (
              <div className="space-y-2">
                <div className="skeleton h-4 rounded" />
                <div className="skeleton h-4 w-2/3 rounded" />
              </div>
            )}
          </Panel>
        </div>
      </div>
    </>
  );
}
