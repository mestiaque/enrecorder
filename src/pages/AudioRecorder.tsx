import { FolderOpen, Mic, RefreshCw } from "lucide-react";
import { AudioSourceSelect } from "../components/recording/AudioSourceSelect";
import { LivePanel } from "../components/recording/LivePanel";
import { MicExtras } from "../components/recording/MicExtras";
import { PageHeader, Panel, Pending, Row, StatusIcon } from "../components/ui/blocks";
import { Button, Segmented, Select, Switch } from "../components/ui/primitives";
import { api, errorMessage } from "../lib/api";
import { useAudioDevices, usePaths, useSystem, useTools } from "../lib/queries";
import type { AudioFormat, Settings } from "../lib/types";
import { audioFrom } from "../store/recorderForm";
import { isActive, useRecording } from "../store/recording";
import { useSettings } from "../store/settings";
import { toast } from "../store/ui";

const FORMATS: { value: AudioFormat; label: string; hint: string }[] = [
  { value: "mp3", label: "MP3", hint: "Plays everywhere" },
  { value: "m4a", label: "M4A (AAC)", hint: "Small, good quality" },
  { value: "opus", label: "Opus", hint: "Best for speech, smallest" },
  { value: "ogg", label: "OGG (Vorbis)", hint: "Open format, plays in most apps" },
  { value: "flac", label: "FLAC", hint: "Lossless, compressed" },
  { value: "wav", label: "WAV", hint: "Lossless, uncompressed" },
];

function SystemPanel() {
  const { data: sys } = useSystem();
  if (!sys) return <Pending label="Detecting…" />;
  return sys.os === "linux" ? (
    <>
      <Row label="PipeWire" hint="Modern Ubuntu audio server">
        <span className="flex items-center gap-1.5">
          <StatusIcon level={sys.pipewire ? "ok" : "warn"} className="h-3.5 w-3.5" />
          {sys.pipewire ? "Running" : "Not running"}
        </span>
      </Row>
      <Row label="PulseAudio interface" hint="ENRecorder records through it (pipewire-pulse or PulseAudio)">
        <span className="flex items-center gap-1.5">
          <StatusIcon level={sys.pulse ? "ok" : "bad"} className="h-3.5 w-3.5" />
          {sys.pulse ? "Available" : "Not available"}
        </span>
      </Row>
    </>
  ) : (
    <Row label="Audio devices" hint="System audio needs a loopback device such as Stereo Mix">
      DirectShow
    </Row>
  );
}

export function AudioRecorderPage() {
  const { settings: s, set } = useSettings();
  const snap = useRecording((st) => st.snap);
  const { data: tools } = useTools();
  const { data: paths } = usePaths();
  const devices = useAudioDevices();
  const active = isActive(snap);
  const both = !!s.audioMic && !!s.audioSystem;
  const blocker = !tools ? "Loading…" : !tools.ffmpeg ? "FFmpeg was not found (Settings → Tools)" : !s.audioMic && !s.audioSystem ? "Choose a microphone or system audio" : null;

  const start = async () => {
    try {
      const list = devices.data ?? (await api.listAudioDevices());
      await api.startAudioRecording({ audio: audioFrom(s.audioMic, s.audioSystem, s.recSeparateTracks, list, s), format: s.audioFormat, quality: s.audioQuality, countdown: 0, maxMinutes: s.recMaxMinutes });
    } catch (e) {
      toast({ kind: "error", title: "Could not start recording", detail: errorMessage(e) }, 10000);
    }
  };

  // Another recording (a screen recording) owns the engine: show it, but do not offer a start.
  if (active && snap && !snap.audioOnly) {
    return (
      <>
        <PageHeader title="Audio Recorder" subtitle="Microphone, system audio, or both." />
        <Panel>
          <p className="py-2 text-[13px] text-muted">A screen recording is running. Stop it before recording audio on its own.</p>
        </Panel>
      </>
    );
  }

  return (
    <>
      <PageHeader title="Audio Recorder" subtitle="Microphone, system audio, or both — mixed or on separate tracks." />
      {active && snap ? (
        <LivePanel snap={snap} />
      ) : (
        <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_380px]">
          <div className="min-w-0 space-y-5">
            <Panel
              title="Sources"
              bodyClass="py-1"
              actions={
                <Button size="sm" variant="ghost" onClick={() => void devices.refetch()} title="Look for devices again">
                  <RefreshCw className="h-3.5 w-3.5" />
                </Button>
              }
            >
              <Row label="Microphone" hint={devices.isError ? errorMessage(devices.error) : undefined}>
                <AudioSourceSelect value={s.audioMic} kind="microphone" devices={devices.data} onChange={(v) => void set("audioMic", v)} />
              </Row>
              <Row label="System audio" hint={devices.data && !devices.data.some((d) => d.kind === "system") ? "No loopback device found" : "Sound playing on this computer"}>
                <AudioSourceSelect value={s.audioSystem} kind="system" devices={devices.data} onChange={(v) => void set("audioSystem", v)} />
              </Row>
              {s.audioMic && (
                <Row label="Reduce background noise" hint="Removes hum, fans and steady hiss from the microphone">
                  <Switch checked={s.recMicDenoise} onChange={(v) => void set("recMicDenoise", v)} label="Reduce background noise" />
                </Row>
              )}
              {s.audioMic && (
                <Row label="Even out the volume" hint="Quiet and loud speech end up at a similar level">
                  <Switch checked={s.recMicNormalize} onChange={(v) => void set("recMicNormalize", v)} label="Even out the volume" />
                </Row>
              )}
              <MicExtras mic={!!s.audioMic} system={!!s.audioSystem} />
              {both && (
                <Row label="Separate tracks" hint="Voice and system sound as two tracks (formats with one track mix them)">
                  <Switch checked={s.recSeparateTracks} onChange={(v) => void set("recSeparateTracks", v)} label="Separate tracks" />
                </Row>
              )}
            </Panel>
            <div className="flex flex-wrap items-center gap-3 rounded-xl border border-line bg-panel p-4">
              <Button variant="primary" className="h-11 px-5 text-[14px]" disabled={!!blocker} onClick={() => void start()}>
                <Mic className="h-4 w-4" />
                Start recording
              </Button>
              <div className="min-w-0 text-[12px] text-muted">{blocker ?? "Recording starts at once; pause and resume as often as you like."}</div>
            </div>
            <Panel title="This computer" bodyClass="py-1">
              <SystemPanel />
            </Panel>
          </div>
          <Panel title="Output" bodyClass="py-1">
            <Row label="Format" hint={FORMATS.find((f) => f.value === s.audioFormat)?.hint}>
              <Select value={s.audioFormat} onChange={(v) => void set("audioFormat", v)} options={FORMATS.map((f) => ({ value: f.value, label: f.label }))} />
            </Row>
            {!["flac", "wav"].includes(s.audioFormat) && (
              <Row label="Quality">
                <Segmented value={s.audioQuality} onChange={(v) => void set("audioQuality", v as Settings["audioQuality"])} options={[{ value: "small", label: "Smaller" }, { value: "balanced", label: "Balanced" }, { value: "high", label: "High" }]} />
              </Row>
            )}
            <Row label="Sample rate">
              <Select value={s.audioSampleRate} onChange={(v) => void set("audioSampleRate", v)} options={[{ value: 44100 as const, label: "44.1 kHz" }, { value: 48000 as const, label: "48 kHz" }, { value: 96000 as const, label: "96 kHz" }]} />
            </Row>
            <Row label="Channels">
              <Segmented value={String(s.audioChannels)} onChange={(v) => void set("audioChannels", Number(v) as 1 | 2)} options={[{ value: "1", label: "Mono" }, { value: "2", label: "Stereo" }]} />
            </Row>
            <Row label="Save to" hint={<span className="font-mono">{paths ? `${paths.recordings}/Audio` : "…"}</span>}>
              <Button size="sm" variant="ghost" disabled={!paths} onClick={() => paths && api.openFolder(paths.recordings).catch((e) => toast({ kind: "error", title: "Could not open the folder", detail: errorMessage(e) }))}>
                <FolderOpen className="h-3.5 w-3.5" />
              </Button>
            </Row>
          </Panel>
        </div>
      )}
    </>
  );
}

