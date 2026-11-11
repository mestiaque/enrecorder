import { describe, expect, it } from "vitest";
import { GB, readinessChecks, shortVersion } from "./readiness";
import type { MediaTools, SystemInfo } from "./types";

const sys = (o: Partial<SystemInfo> = {}): SystemInfo => ({
  os: "linux",
  osName: "Ubuntu 22.04",
  arch: "x86_64",
  displayServer: "x11",
  desktop: "GNOME",
  pipewire: true,
  pulse: true,
  renderNodes: [],
  monitors: [],
  ...o,
});
const tool = { path: "/usr/bin/ffmpeg", version: "4.4.2-0ubuntu0.22.04.1" };
const tools = (o: Partial<MediaTools> = {}): MediaTools => ({ ffmpeg: tool, ffprobe: tool, capabilities: null, checking: false, ...o });
const find = (cs: ReturnType<typeof readinessChecks>, id: string) => cs.find((c) => c.id === id);

describe("readiness", () => {
  it("is pending while loading", () => {
    const cs = readinessChecks(undefined, undefined, undefined);
    expect(cs.every((c) => c.level === "pending")).toBe(true);
  });

  it("wayland without pipewire cannot capture", () => {
    expect(find(readinessChecks(sys({ displayServer: "wayland", pipewire: false }), tools(), null), "display")?.level).toBe("bad");
    expect(find(readinessChecks(sys({ displayServer: "wayland" }), tools(), null), "display")?.detail).toContain("Wayland portal");
  });

  it("reports missing ffmpeg and verified encoders only", () => {
    expect(find(readinessChecks(sys(), tools({ ffmpeg: null }), null), "ffmpeg")?.level).toBe("bad");
    const caps = {
      ffmpeg: tool,
      checkedAt: 1,
      encoders: [
        { name: "libx264", codec: "h264" as const, vendor: "software" as const, listed: true, verified: true, detail: "" },
        { name: "h264_vaapi", codec: "h264" as const, vendor: "vaapi" as const, listed: true, verified: true, detail: "" },
        { name: "h264_nvenc", codec: "h264" as const, vendor: "nvidia" as const, listed: true, verified: false, detail: "no device" },
      ],
    };
    const enc = find(readinessChecks(sys(), tools({ capabilities: caps }), null), "encoders");
    expect(enc?.level).toBe("ok");
    expect(enc?.detail).toContain("2 verified");
    expect(enc?.detail).toContain("h264_vaapi");
    expect(enc?.detail).not.toContain("nvenc");
  });

  it("warns about low disk space", () => {
    expect(find(readinessChecks(sys(), tools(), { path: "/", available: 1 * GB, total: 100 * GB }), "disk")?.level).toBe("warn");
    expect(find(readinessChecks(sys(), tools(), { path: "/", available: 0.2 * GB, total: 100 * GB }), "disk")?.level).toBe("bad");
    expect(find(readinessChecks(sys(), tools(), { path: "/", available: 50 * GB, total: 100 * GB }), "disk")?.level).toBe("ok");
  });

  it("shortens versions", () => {
    expect(shortVersion("4.4.2-0ubuntu0.22.04.1")).toBe("4.4.2");
    expect(shortVersion("n6.1")).toBe("6.1");
    expect(shortVersion("N-112345-gabc")).toBe("N-112345-gabc");
  });
});
