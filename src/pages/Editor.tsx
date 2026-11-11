import { PageHeader, Panel, Pending, PhaseNotice, Row, StatusIcon } from "../components/ui/blocks";
import { useTools } from "../lib/queries";
import { shortVersion } from "../lib/readiness";

export function EditorPage() {
  const { data: tools } = useTools();
  return (
    <>
      <PageHeader title="Media Editor" subtitle="Trim, split, crop and convert recordings without touching the original." />
      <PhaseNotice phase={5} title="The editor is being built">
        The preview player, timeline, trimming and the export settings arrive in phase 5. Edits are non-destructive: the original recording is never overwritten.
      </PhaseNotice>
      <Panel title="Media tools" bodyClass="py-1" className="max-w-2xl">
        {!tools ? (
          <Pending label="Loading…" />
        ) : (
          <>
            <Row label="FFmpeg" hint={tools.ffmpeg?.path ?? "Converts and exports media"}>
              <span className="flex items-center gap-1.5">
                <StatusIcon level={tools.ffmpeg ? "ok" : tools.checking ? "pending" : "bad"} className="h-3.5 w-3.5" />
                {tools.ffmpeg ? shortVersion(tools.ffmpeg.version) : tools.checking ? "Looking…" : "Not found"}
              </span>
            </Row>
            <Row label="FFprobe" hint={tools.ffprobe?.path ?? "Reads codecs, duration and streams"}>
              <span className="flex items-center gap-1.5">
                <StatusIcon level={tools.ffprobe ? "ok" : tools.checking ? "pending" : "warn"} className="h-3.5 w-3.5" />
                {tools.ffprobe ? shortVersion(tools.ffprobe.version) : tools.checking ? "Looking…" : "Not found"}
              </span>
            </Row>
          </>
        )}
      </Panel>
    </>
  );
}
