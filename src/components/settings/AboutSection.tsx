import { getVersion } from "@tauri-apps/api/app";
import { Copy, ExternalLink, FolderOpen } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import appIcon from "../../assets/enrecorder-icon.svg";
import { api, errorMessage } from "../../lib/api";
import { usePaths, useSystem, useTools } from "../../lib/queries";
import { shortVersion } from "../../lib/readiness";
import type { AboutInfo } from "../../lib/types";
import { toast } from "../../store/ui";
import { Button } from "../ui/primitives";

const LIBRARIES: [string, string, string][] = [
  ["Tauri", "desktop shell", "MIT / Apache-2.0"],
  ["FFmpeg", "encoding and conversion", "LGPL / GPL, separate program"],
  ["Rust · rusqlite · uuid", "engine and storage", "MIT / Apache-2.0"],
  ["SQLite", "library and settings", "public domain"],
  ["React · Radix UI · Zustand · TanStack Query", "interface", "MIT"],
  ["Tailwind CSS · Lucide", "styling · icons", "MIT · ISC"],
];

const Section = ({ title, children }: { title: string; children: ReactNode }) => (
  <section>
    <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-faint">{title}</div>
    <div className="divide-y divide-line rounded-lg bg-raised px-3">{children}</div>
  </section>
);
const Row = ({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children?: ReactNode }) => (
  <div className="flex items-center justify-between gap-4 py-2.5">
    <span className="min-w-0">
      {label}
      {hint && <span className="block truncate text-[11px] text-faint">{hint}</span>}
    </span>
    {children}
  </div>
);

const DISPLAY: Record<string, string> = { x11: "X11", wayland: "Wayland", windows: "Windows", unknown: "none" };

export function AboutSection() {
  const [version, setVersion] = useState("");
  const [about, setAbout] = useState<AboutInfo | null>(null);
  const { data: paths } = usePaths();
  const { data: sys } = useSystem();
  const { data: tools } = useTools();
  useEffect(() => {
    getVersion().then(setVersion, () => {});
    api.aboutInfo().then(setAbout, () => {});
  }, []);
  const fail = (what: string) => (e: unknown) => toast({ kind: "error", title: `Could not open ${what}`, detail: errorMessage(e) });
  const mono = "font-mono text-[12px] text-muted";

  return (
    <div className="space-y-5 py-4 text-[13px]">
      <div className="flex items-center gap-4">
        <img src={appIcon} alt="" className="h-16 w-16" draggable={false} />
        <div>
          <div className="text-lg font-semibold">
            ENRecorder <span className="font-normal text-muted">{version}</span>
          </div>
          <div className="text-muted">One studio. Every capture.</div>
          <div className="mt-1 text-[11px] text-faint">Screen, audio and screenshots — local-first, no account, no upload, no watermark.</div>
        </div>
      </div>

      <div>
        <Section title="This copy">
          <Row label="Version">
            <span className={mono}>
              {version} {about?.build === "debug" && <span className="ml-1 rounded bg-amber-500/20 px-1 text-amber-500">debug build</span>}
            </span>
          </Row>
          <Row label="System">
            <span className="text-right text-muted">{sys ? `${sys.osName} · ${sys.arch} · ${DISPLAY[sys.displayServer]}${sys.desktop ? ` (${sys.desktop})` : ""}` : "…"}</span>
          </Row>
          <Row label="Tauri · WebView">
            <span className={mono}>{about ? `${about.tauriVersion} · ${about.webviewVersion || "unknown"}` : "…"}</span>
          </Row>
          <Row label="FFmpeg" hint="Settings → Tools">
            <span className={mono}>{tools?.ffmpeg ? shortVersion(tools.ffmpeg.version) : tools?.checking ? "…" : "not found"}</span>
          </Row>
          <Row label="Library and settings" hint={paths?.data}>
            <Button size="sm" disabled={!paths} onClick={() => paths && api.openPath(paths.data).catch(fail("the data folder"))}>
              <FolderOpen className="h-3.5 w-3.5" />
              Open
            </Button>
          </Row>
          <Row label="Logs" hint={paths?.logs}>
            <Button size="sm" onClick={() => api.openLogFolder().catch(fail("the logs folder"))}>
              <FolderOpen className="h-3.5 w-3.5" />
              Open
            </Button>
          </Row>
        </Section>
        <div className="mt-2 flex justify-end">
          <Button
            size="sm"
            variant="ghost"
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(await api.diagnostics());
                toast({ kind: "success", title: "Diagnostics copied", detail: "Paste them into your bug report" });
              } catch (e) {
                toast({ kind: "error", title: "Could not collect diagnostics", detail: errorMessage(e) });
              }
            }}
          >
            <Copy className="h-3.5 w-3.5" />
            Copy diagnostics
          </Button>
        </div>
      </div>

      <Section title="Privacy">
        <Row label="Recordings stay on this computer" hint="No account, no cloud upload, no telemetry" />
        <Row label="Nothing is captured without you" hint="Recording always shows an indicator; on Wayland your desktop asks first" />
      </Section>

      <Section title="Developer">
        <Row label="Developed by">
          <span className="text-muted">M. Estiaque Ahmed Khan</span>
        </Row>
        <Row label="Contact" hint="Bug reports, ideas and feedback are welcome">
          <button className="text-accent hover:underline" onClick={() => api.openUrl("mailto:info@mestiaque.com").catch(() => toast({ kind: "error", title: "No mail app found", detail: "info@mestiaque.com" }))}>
            info@mestiaque.com
          </button>
        </Row>
        <Row label="Website">
          <button className="flex items-center gap-1 text-accent hover:underline" onClick={() => api.openUrl("https://mestiaque.com").catch(fail("the website"))}>
            mestiaque.com
            <ExternalLink className="h-3 w-3" />
          </button>
        </Row>
        <Row label="Also by the developer" hint="Same look and themes">
          <span className="text-muted">ENViewer · ENPlayer · ENSave · ENStream · ENiShare · ENPost</span>
        </Row>
        <Row label="Licence">
          <span className="text-muted">© 2026 M. Estiaque Ahmed Khan · MIT</span>
        </Row>
      </Section>

      <Section title="Built with">
        {LIBRARIES.map(([name, what, licence]) => (
          <div key={name} className="flex items-center justify-between gap-4 py-2 text-[12px]">
            <span>
              {name} <span className="text-faint">· {what}</span>
            </span>
            <span className="text-faint">{licence}</span>
          </div>
        ))}
      </Section>
    </div>
  );
}
