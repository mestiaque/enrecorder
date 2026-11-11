#!/usr/bin/env bash
# Headless Audio Recorder test (inside enrec-build): PipeWire with a virtual speaker (its
# monitor is system audio, playing 440 Hz) and a virtual microphone (pipe source, 880 Hz);
# open the Audio Recorder (Ctrl+3), start, pause/resume, stop; check the file and the library.
#   STEP=shot  → only screenshot the page (to find the buttons)
set -uo pipefail
OUT="${1:-/src/target/smoke-audio}"
BIN="${BIN:-/src/target/debug/enrecorder}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run XDG_DATA_HOME=/tmp/home/.local/share XDG_CONFIG_HOME=/tmp/home/.config
rm -rf /tmp/home/.local/share/com.mestiaque.enrecorder /tmp/home/Videos/ENRecorder
mkdir -p "$XDG_RUNTIME_DIR" "$HOME/Videos" "$HOME/Pictures" && chmod 700 "$XDG_RUNTIME_DIR"
eval "$(dbus-launch --sh-syntax)"
pipewire >/dev/null 2>&1 & sleep 1; wireplumber >/dev/null 2>&1 & sleep 1; pipewire-pulse >/dev/null 2>&1 & sleep 1.5
pactl load-module module-null-sink sink_name=speaker sink_properties=device.description=Speakers >/dev/null; pactl set-default-sink speaker
rm -f /tmp/micpipe
pactl load-module module-pipe-source source_name=testmic file=/tmp/micpipe format=s16le rate=48000 channels=2 source_properties=device.description=Headset_Microphone >/dev/null
pactl set-default-source testmic
ffmpeg -hide_banner -loglevel error -re -f lavfi -i "sine=frequency=440:sample_rate=48000" -t 300 -f pulse -device speaker tone &
ffmpeg -hide_banner -loglevel error -re -f lavfi -i "sine=frequency=880:sample_rate=48000" -t 300 -ac 2 -f s16le -y /tmp/micpipe &
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 &
export DISPLAY=:99 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
sleep 1
"$BIN" >"$OUT/app.log" 2>&1 &
APP=$!
trap 'kill $APP 2>/dev/null; pkill ffmpeg; pkill -f "pipewire|wireplumber|Xvfb"' EXIT
sleep 22
xdotool mousemove 720 450 click 1
xdotool key ctrl+3; sleep 2.5
import -window root "$OUT/1-audio-page.png"
[ "${STEP:-}" = shot ] && exit 0
xdotool mousemove "$START_X" "$START_Y" click 1; sleep 2.5
import -window root "$OUT/2-recording.png"
xdotool key ctrl+shift+p; sleep 1.5; xdotool key ctrl+shift+p; sleep 2
import -window root "$OUT/3-resumed.png"
xdotool key ctrl+shift+r; sleep 4
import -window root "$OUT/4-done.png"
echo "--- files ---"; ls -la /tmp/home/Videos/ENRecorder/Audio 2>&1
for f in /tmp/home/Videos/ENRecorder/Audio/*; do
  [ -f "$f" ] && ffprobe -v error -show_entries format=duration:stream=codec_name,channels -of compact "$f" && ffmpeg -hide_banner -nostats -i "$f" -af volumedetect -f null - 2>&1 | grep max_volume
done
python3 - <<'PY'
import sqlite3
c = sqlite3.connect('/tmp/home/.local/share/com.mestiaque.enrecorder/enrecorder.db')
print("recordings:", c.execute("select kind, title, duration_ms, status from recordings").fetchall())
print("sessions:", c.execute("select kind, state, error from capture_sessions").fetchall())
PY
grep -E "ERROR|WARN" "$OUT/app.log" | grep -v "encoder\|clipboard" | tail -5
