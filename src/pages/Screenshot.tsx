import { FolderOpen } from "lucide-react";
import { PageHeader, Panel, PhaseNotice, Row } from "../components/ui/blocks";
import { Button } from "../components/ui/primitives";
import { api, errorMessage } from "../lib/api";
import { usePaths, useSystem } from "../lib/queries";
import { toast } from "../store/ui";

export function ScreenshotPage() {
  const { data: paths } = usePaths();
  const { data: sys } = useSystem();
  return (
    <>
      <PageHeader title="Screenshot Studio" subtitle="Capture the screen, a window or a region, then annotate and export." />
      <PhaseNotice phase={4} title="Screenshots are being built">
        Capture modes, the annotation editor (arrows, text, blur, steps…) and PNG/JPEG/WebP export arrive in phase 4.
      </PhaseNotice>
      <Panel title="Where screenshots go" bodyClass="py-1" className="max-w-2xl">
        <Row label="Save folder" hint={paths?.screenshots ?? "…"}>
          <Button
            size="sm"
            disabled={!paths}
            onClick={() => paths && api.openFolder(paths.screenshots).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) }))}
          >
            <FolderOpen className="h-3.5 w-3.5" />
            Open
          </Button>
        </Row>
        <Row label="Displays">{sys ? `${sys.monitors.length} connected` : "…"}</Row>
        <Row label="Change the folder" hint="Settings → Storage" />
      </Panel>
    </>
  );
}
