/**
 * Video encoders, grouped by codec. Only a passed test encode counts as available. Hardware this
 * computer does not have is normal, so it is shown as "Not available" (grey, folded away), not as
 * an error; red is kept for a software encoder that fails, which would be a real problem.
 */
import { ChevronDown, MinusCircle, RefreshCw } from "lucide-react";
import { useState } from "react";
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

type RowState = { level: "ok" | "off" | "bad"; text: string; reason?: string };

/** Why a hardware encoder cannot be used here, in words a user understands. */
function reason(e: EncoderStatus): string {
  if (e.detail.startsWith("Skipped")) return "The graphics hardware did not respond to the test.";
  switch (e.vendor) {
    case "nvidia":
      return "Needs an NVIDIA graphics card (with its driver).";
    case "amd":
      return "Needs an AMD graphics card (with its driver).";
    case "intel":
      return "Intel Quick Sync is not available on this computer.";
    case "vaapi":
      return "This graphics card or its driver cannot encode this format.";
    case "mediaFoundation":
      return "Windows offers no hardware encoder for this format here.";
    default:
      return "The test encode failed.";
  }
}

function state(e: EncoderStatus): RowState {
  if (e.verified) return { level: "ok", text: "Ready" };
  if (e.vendor === "software") return { level: "bad", text: "Not working", reason: "This FFmpeg could not encode with it. Choose another FFmpeg in Settings → Tools." };
  return { level: "off", text: "Not available", reason: reason(e) };
}

function Row({ e }: { e: EncoderStatus }) {
  const st = state(e);
  const label = (
    <span className={st.level === "ok" ? "text-success" : st.level === "bad" ? "text-danger" : "text-faint"}>{st.text}</span>
  );
  return (
    <div className="flex items-center gap-2 border-b border-line/60 py-1.5 text-[12px] last:border-0">
      {st.level === "off" ? <MinusCircle className="h-3.5 w-3.5 shrink-0 text-faint" /> : <StatusIcon level={st.level} className="h-3.5 w-3.5" />}
      <span className={st.level === "off" ? "font-mono text-faint" : "font-mono"}>{e.name}</span>
      <span className="flex-1 truncate text-faint">{VENDOR[e.vendor]}</span>
      {st.reason ? (
        <Tip
          label={
            <span className="block max-w-[360px] text-[12px]">
              {st.reason}
              {e.detail && <span className="mt-1 block whitespace-pre-wrap font-mono text-[10px] opacity-70">{e.detail}</span>}
            </span>
          }
        >
          <span className="cursor-help">{label}</span>
        </Tip>
      ) : (
        label
      )}
    </div>
  );
}

export function EncoderTable() {
  const { data: tools } = useTools();
  const caps = tools?.capabilities;
  const [showAll, setShowAll] = useState(false);
  const hidden = caps?.encoders.filter((e) => e.listed && !e.verified && e.vendor !== "software").length ?? 0;
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
              const rows = caps.encoders.filter((e) => e.codec === codec && e.listed && (showAll || e.verified || e.vendor === "software"));
              const anyListed = caps.encoders.some((e) => e.codec === codec && e.listed);
              return (
                <div key={codec}>
                  <div className="mb-1 text-[11px] font-semibold uppercase tracking-wider text-faint">{label}</div>
                  {!anyListed ? (
                    <div className="py-1.5 text-[12px] text-faint">Not in this FFmpeg build</div>
                  ) : rows.length === 0 ? (
                    <div className="py-1.5 text-[12px] text-faint">No encoder for this format works here</div>
                  ) : (
                    rows.map((e) => <Row key={e.name} e={e} />)
                  )}
                </div>
              );
            })}
          </div>
          {hidden > 0 && (
            <button className="mt-2 flex items-center gap-1 text-[12px] text-muted hover:text-fg" onClick={() => setShowAll(!showAll)}>
              <ChevronDown className={showAll ? "h-3.5 w-3.5 rotate-180 transition-transform" : "h-3.5 w-3.5 transition-transform"} />
              {showAll ? "Hide hardware this computer does not have" : `Show ${hidden} hardware encoder${hidden > 1 ? "s" : ""} not available on this computer`}
            </button>
          )}
          <div className="mt-3 text-[11px] text-faint">
            Nothing to set up: ENRecorder records with the best encoder that is ready (graphics card first, otherwise the processor). Each one was tested with a short sample using FFmpeg {caps.ffmpeg.version} on{" "}
            {formatDate(caps.checkedAt)}.
          </div>
        </>
      )}
    </Panel>
  );
}
