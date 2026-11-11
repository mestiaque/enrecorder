import { describe, expect, it } from "vitest";
import { formatBytes, formatDuration, formatTotalTime } from "./format";

describe("format", () => {
  it("durations", () => {
    expect(formatDuration(65000)).toBe("1:05");
    expect(formatDuration(3725000)).toBe("1:02:05");
    expect(formatDuration(null)).toBe("—");
  });
  it("total time", () => {
    expect(formatTotalTime(0)).toBe("0 min");
    expect(formatTotalTime(125 * 60000)).toBe("2 h 5 min");
    expect(formatTotalTime(120 * 60000)).toBe("2 h");
  });
  it("bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
  });
});
