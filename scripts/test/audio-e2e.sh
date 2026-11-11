#!/usr/bin/env bash
# Audio end-to-end in enrec-build: PipeWire + WirePlumber + pipewire-pulse, a virtual speaker
# playing a tone (its monitor is "system audio"), a virtual microphone fed from a second tone,
# then the record_audio example records both and FFprobe checks the files.
set -euo pipefail
# Build first: the test tones must still be playing when recording starts.
cargo build -q -p enrec-core --example record_audio
export XDG_RUNTIME_DIR=/tmp/xdg; mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
eval "$(dbus-launch --sh-syntax)"
pipewire >/tmp/pw.log 2>&1 & sleep 1
wireplumber >/tmp/wp.log 2>&1 & sleep 1
pipewire-pulse >/tmp/pwp.log 2>&1 & sleep 1.5
pactl load-module module-null-sink sink_name=speaker sink_properties=device.description=Speaker >/dev/null
pactl set-default-sink speaker
# A virtual microphone: a pipe source fed with a second tone.
rm -f /tmp/micpipe
pactl load-module module-pipe-source source_name=testmic file=/tmp/micpipe format=s16le rate=48000 channels=2 source_properties=device.description=TestMic >/dev/null
pactl set-default-source testmic
ffmpeg -hide_banner -loglevel error -re -f lavfi -i "sine=frequency=440:sample_rate=48000" -t 120 -f pulse -device speaker "tone" &
ffmpeg -hide_banner -loglevel error -re -f lavfi -i "sine=frequency=880:sample_rate=48000" -t 120 -ac 2 -f s16le -y /tmp/micpipe &
sleep 1
OUT=$(mktemp -d)
timeout 120 target/debug/examples/record_audio "$OUT"
for f in "$OUT"/*; do
  echo "== $(basename "$f")"
  ffprobe -v error -show_entries stream=codec_name,channels -show_entries format=duration -of compact "$f"
  ffmpeg -hide_banner -nostats -i "$f" -map 0:a:0 -af volumedetect -f null - 2>&1 | grep -E "mean_volume|max_volume"
done
pkill ffmpeg || true
pkill -f "pipewire|wireplumber" || true
