import type { AudioDevice } from "../../lib/types";
import { Select } from "../ui/primitives";

/** "" = off, "default" = the system default, otherwise a device id. */
export function AudioSourceSelect({ value, kind, devices, onChange, allowOff = true }: { value: string; kind: AudioDevice["kind"]; devices: AudioDevice[] | undefined; onChange: (v: string) => void; allowOff?: boolean }) {
  const list = (devices ?? []).filter((d) => d.kind === kind);
  const known = value === "" || value === "default" || list.some((d) => d.id === value);
  const options = [
    ...(allowOff ? [{ value: "", label: "Off" }] : []),
    { value: "default", label: kind === "microphone" ? "Default microphone" : "Default output (what you hear)" },
    ...list.map((d) => ({ value: d.id, label: d.name })),
    ...(known ? [] : [{ value, label: "Disconnected device" }]),
  ];
  return <Select value={value} onChange={onChange} className="max-w-[260px]" options={options} />;
}
