import { useSystem } from "../../lib/queries";
import { useSettings } from "../../store/settings";
import { Row } from "../ui/blocks";
import { Slider, Switch } from "../ui/primitives";

/** Input volumes, noise gate and echo cancellation (shared with the Video Recorder). */
export function MicExtras({ mic, system }: { mic: boolean; system: boolean }) {
  const { settings: s, set } = useSettings();
  const { data: sys } = useSystem();
  return (
    <>
      {mic && (
        <>
          <Row label="Microphone volume" hint={`${s.recMicVolume} %`}>
            <div className="w-40">
              <Slider value={s.recMicVolume} min={0} max={200} step={5} onChange={(v) => useSettings.setState({ settings: { ...s, recMicVolume: v } })} onCommit={(v) => void set("recMicVolume", v)} />
            </div>
          </Row>
          <Row label="Noise gate" hint="Silences the microphone between words (keyboard, breathing)">
            <Switch checked={s.recMicGate} onChange={(v) => void set("recMicGate", v)} label="Noise gate" />
          </Row>
          {sys?.os === "linux" && (
            <Row label="Echo cancellation" hint="Removes speaker sound that the microphone picks up (WebRTC, through PipeWire/PulseAudio)">
              <Switch checked={s.recEchoCancel} onChange={(v) => void set("recEchoCancel", v)} label="Echo cancellation" />
            </Row>
          )}
        </>
      )}
      {system && (
        <Row label="System audio volume" hint={`${s.recSystemVolume} %`}>
          <div className="w-40">
            <Slider value={s.recSystemVolume} min={0} max={200} step={5} onChange={(v) => useSettings.setState({ settings: { ...s, recSystemVolume: v } })} onCommit={(v) => void set("recSystemVolume", v)} />
          </div>
        </Row>
      )}
    </>
  );
}
