import { PageHeader, Panel, Pending, PhaseNotice, Row, StatusIcon } from "../components/ui/blocks";
import { useSystem } from "../lib/queries";

export function AudioRecorderPage() {
  const { data: sys } = useSystem();
  return (
    <>
      <PageHeader title="Audio Recorder" subtitle="Microphone, system audio, or both on separate tracks." />
      <PhaseNotice phase={3} title="Audio recording is being built">
        Device lists, live level meters and multi-track recording (CPAL, WASAPI loopback, PipeWire) arrive in phase 3. Nothing is recorded yet.
      </PhaseNotice>
      <Panel title="Audio system" bodyClass="py-1" className="max-w-2xl">
        {!sys ? (
          <Pending label="Detecting…" />
        ) : sys.os === "linux" ? (
          <>
            <Row label="PipeWire" hint="Needed for system audio on Wayland and for screen sharing">
              <span className="flex items-center gap-1.5">
                <StatusIcon level={sys.pipewire ? "ok" : "warn"} className="h-3.5 w-3.5" />
                {sys.pipewire ? "Running" : "Not running"}
              </span>
            </Row>
            <Row label="PulseAudio interface" hint="pipewire-pulse or PulseAudio">
              <span className="flex items-center gap-1.5">
                <StatusIcon level={sys.pulse ? "ok" : "warn"} className="h-3.5 w-3.5" />
                {sys.pulse ? "Available" : "Not available"}
              </span>
            </Row>
          </>
        ) : (
          <Row label="Audio API">WASAPI (microphone and loopback)</Row>
        )}
      </Panel>
    </>
  );
}
