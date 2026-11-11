/** System readiness checks shown on Home and in the recorder pages. Pure, unit-tested. */
import type { DiskSpace, MediaTools, SystemInfo } from "./types";

export type CheckLevel = "ok" | "warn" | "bad" | "pending";

export interface Check {
  id: string;
  label: string;
  level: CheckLevel;
  detail: string;
}

export const GB = 1024 ** 3;

export function captureBackendLabel(sys: SystemInfo): string {
  switch (sys.displayServer) {
    case "x11":
      return "X11 screen capture";
    case "wayland":
      return "Wayland portal + PipeWire";
    case "windows":
      return "Windows.Graphics.Capture";
    default:
      return "No graphical session";
  }
}

export function readinessChecks(sys: SystemInfo | undefined, tools: MediaTools | undefined, disk: DiskSpace | null | undefined): Check[] {
  const checks: Check[] = [];
  if (!sys) checks.push({ id: "display", label: "Screen capture", level: "pending", detail: "Detecting…" });
  else {
    const desktop = sys.desktop ? ` on ${sys.desktop}` : "";
    if (sys.displayServer === "unknown") checks.push({ id: "display", label: "Screen capture", level: "bad", detail: "No X11 or Wayland session was found" });
    else if (sys.displayServer === "wayland" && sys.pipewire === false)
      checks.push({ id: "display", label: "Screen capture", level: "bad", detail: `Wayland${desktop}: PipeWire is not running, so the screen cannot be shared` });
    else checks.push({ id: "display", label: "Screen capture", level: "ok", detail: `${captureBackendLabel(sys)}${desktop}` });

    if (sys.os === "linux") {
      if (sys.pipewire) checks.push({ id: "audio", label: "Audio server", level: "ok", detail: sys.pulse ? "PipeWire (with PulseAudio support)" : "PipeWire" });
      else if (sys.pulse) checks.push({ id: "audio", label: "Audio server", level: "warn", detail: "PulseAudio (system audio capture needs PipeWire on Wayland)" });
      else checks.push({ id: "audio", label: "Audio server", level: "bad", detail: "Neither PipeWire nor PulseAudio answers" });
    } else if (sys.os === "windows") checks.push({ id: "audio", label: "Audio server", level: "ok", detail: "WASAPI" });
  }

  if (!tools) checks.push({ id: "ffmpeg", label: "FFmpeg", level: "pending", detail: "Looking for FFmpeg…" });
  else if (!tools.ffmpeg) checks.push({ id: "ffmpeg", label: "FFmpeg", level: tools.checking ? "pending" : "bad", detail: tools.checking ? "Looking for FFmpeg…" : "Not found — recording and export need it" });
  else {
    checks.push({ id: "ffmpeg", label: "FFmpeg", level: "ok", detail: `Version ${shortVersion(tools.ffmpeg.version)}` });
    checks.push(
      tools.ffprobe
        ? { id: "ffprobe", label: "FFprobe", level: "ok", detail: `Version ${shortVersion(tools.ffprobe.version)}` }
        : { id: "ffprobe", label: "FFprobe", level: "warn", detail: "Not found — media details in the library need it" },
    );
    const caps = tools.capabilities;
    if (!caps) checks.push({ id: "encoders", label: "Video encoders", level: tools.checking ? "pending" : "warn", detail: tools.checking ? "Testing encoders…" : "Not tested" });
    else {
      const ok = caps.encoders.filter((e) => e.verified);
      const hw = ok.filter((e) => e.vendor !== "software");
      if (!ok.length) checks.push({ id: "encoders", label: "Video encoders", level: "bad", detail: "No encoder passed its test" });
      else
        checks.push({
          id: "encoders",
          label: "Video encoders",
          level: "ok",
          detail: hw.length ? `${ok.length} verified, ${hw.length} hardware (${hw.map((e) => e.name).join(", ")})` : `${ok.length} verified (software only)`,
        });
    }
  }

  if (disk) {
    const free = disk.available;
    const level: CheckLevel = free < 0.5 * GB ? "bad" : free < 2 * GB ? "warn" : "ok";
    checks.push({ id: "disk", label: "Disk space", level, detail: `${(free / GB).toFixed(free < 10 * GB ? 1 : 0)} GB free${level === "ok" ? "" : " — recordings may stop early"}` });
  }
  return checks;
}

/** "4.4.2-0ubuntu0.22.04.1" → "4.4.2"; git builds stay as they are. */
export function shortVersion(v: string): string {
  const m = v.match(/^n?(\d+\.\d+(?:\.\d+)?)/);
  return m ? m[1] : v;
}
