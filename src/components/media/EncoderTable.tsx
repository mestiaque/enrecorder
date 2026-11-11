/** Verified video encoders, grouped by codec. Only a passed test encode counts as available. */
import { RefreshCw } from "lucide-react";
import { Panel, Pending, StatusIcon } from "../ui/blocks";
import { Button, Tip } from "../ui/primitives";
import { api, errorMessage } from "../../lib/api";
import { formatDate } from "../../lib/format";
import { useTools } from "../../lib/queries";
import type { Codec, EncoderStatus, Vendor } from "../../lib/types";
import { toast } from "../../store/ui";

const CODECS: [Codec, string][] = [
  ["h264", "H.264"],
  ["hevc", "H.265 / HEVC"],
  ["av1", "AV1"],
  ["vp9", "VP9 (WebM)"],
];

export const VENDOR: Record<Vendor, string> = {
  software: "Software (CPU)",
  nvidia: "NVIDIA NVENC",
  amd: "AMD AMF",
  intel: "Intel Quick Sync",
  vaapi: "VA-API (GPU)",
  mediaFoundation: "Media Foundation",
};

function state(e: EncoderStatus) {
  if (e.verified) return { level: "ok" as const, text: "Verified" };
  if (e.listed) return { level: "bad" as const, text: "Test failed" };
  return null;
}

export function EncoderTable() {
  const { data: tools } = useTools();
  const caps = tools?.capabilities;
  const retest = () => api.verifyEncoders().catch((e) => toast({ kind: "error", title: "Could not start the test", detail: errorMessage(e) }));
  return (
    <Panel
      title="Video encoders"
      actions={
        <Button size="sm" variant="ghost" disabled={!tools?.ffmpeg || tools.checking} onClick={() => void retest()}>
          <RefreshCw className={tools?.checking ? "h-3.5 w-3.5 animate-spin" : "h-3.5 w-3.5"} />
          {tools?.checking ? "Testing…" : "Test again"}
        </Button>
      }
    >
      {!tools ? (
        <Pending label="Loading…" />
      ) : !tools.ffmpeg ? (
        <div className="text-[13px] text-muted">{tools.checking ? "Looking for FFmpeg…" : "FFmpeg was not found, so no encoder can be used. Install it (Ubuntu: sudo apt install ffmpeg) or choose it in Settings → Tools."}</div>
      ) : !caps ? (
        <Pending label="Testing each encoder with a short sample… (a few seconds)" />
      ) : (
        <>
          <div className="grid gap-4 md:grid-cols-2">
            {CODECS.map(([codec, label]) => {
              const rows = caps.encoders.filter((e) => e.codec === codec && e.listed);
              return (
                <div key={codec}>
                  <div className="mb-1 text-[11px] font-semibold uppercase tracking-wider text-faint">{label}</div>
                  {rows.length === 0 ? (
                    <div className="py-1.5 text-[12px] text-faint">Not in this FFmpeg build</div>
                  ) : (
                    rows.map((e) => {
                      const st = state(e);
                      return (
                        <div key={e.name} className="flex items-center gap-2 border-b border-line/60 py-1.5 text-[12px] last:border-0">
                          {st && <StatusIcon level={st.level} className="h-3.5 w-3.5" />}
                          <span className="font-mono">{e.name}</span>
                          <span className="flex-1 truncate text-faint">{VENDOR[e.vendor]}</span>
                          {e.detail ? (
                            <Tip label={<span className="block max-w-[360px] whitespace-pre-wrap font-mono text-[11px]">{e.detail}</span>}>
                              <span className="cursor-help text-danger underline decoration-dotted">{st?.text}</span>
                            </Tip>
                          ) : (
                            <span className="text-success">{st?.text}</span>
                          )}
                        </div>
                      );
                    })
                  )}
                </div>
              );
            })}
          </div>
          <div className="mt-3 text-[11px] text-faint">
            Tested with FFmpeg {caps.ffmpeg.version} on {formatDate(caps.checkedAt)}. An encoder counts as available only after a real test encode on this computer.
          </div>
        </>
      )}
    </Panel>
  );
}
