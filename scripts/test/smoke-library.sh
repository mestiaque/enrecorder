#!/usr/bin/env bash
# Library / export / editor UI test (enrec-build, Xvfb + PipeWire): record 3 s of audio, open it in
# Recordings (detail panel with thumbnail and player), export it as M4A through the dialog,
# watch the Export Center, open the Media Editor.  Coordinates from the 1280×800 layout.
set -uo pipefail
OUT="${1:-/src/target/smoke-library}"
BIN="${BIN:-/src/target/debug/enrecorder}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run XDG_DATA_HOME=/tmp/home/.local/share XDG_CONFIG_HOME=/tmp/home/.config
rm -rf /tmp/home/.local/share/com.mestiaque.enrecorder /tmp/home/Videos/ENRecorder
mkdir -p "$XDG_RUNTIME_DIR" "$HOME/Videos" "$HOME/Pictures" && chmod 700 "$XDG_RUNTIME_DIR"
eval "$(dbus-launch --sh-syntax)"
pipewire >/dev/null 2>&1 & sleep 1; wireplumber >/dev/null 2>&1 & sleep 1; pipewire-pulse >/dev/null 2>&1 & sleep 1.5
pactl load-module module-null-sink sink_name=speaker >/dev/null; pactl set-default-sink speaker
rm -f /tmp/micpipe
pactl load-module module-pipe-source source_name=testmic file=/tmp/micpipe format=s16le rate=48000 channels=2 source_properties=device.description=Microphone >/dev/null
pactl set-default-source testmic
ffmpeg -hide_banner -loglevel error -re -f lavfi -i "sine=frequency=660:sample_rate=48000" -t 300 -ac 2 -f s16le -y /tmp/micpipe &
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 &
export DISPLAY=:99 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
sleep 1
"$BIN" >"$OUT/app.log" 2>&1 &
APP=$!
trap 'kill $APP 2>/dev/null; pkill ffmpeg; pkill -f "pipewire|wireplumber|Xvfb"' EXIT
sleep 22
xdotool mousemove 720 450 click 1
xdotool key ctrl+3; sleep 2
xdotool mousemove 425 416 click 1; sleep 3.5; xdotool key ctrl+shift+r; sleep 4
xdotool key ctrl+6; sleep 2.5
import -window root "$OUT/1-recordings.png"
xdotool mousemove "${ROW_X:-600}" "${ROW_Y:-265}" click 1; sleep 2.5
import -window root "$OUT/2-detail.png"
[ "${STEP:-}" = detail ] && exit 0
xdotool mousemove "$EXPORT_X" "$EXPORT_Y" click 1; sleep 1.5
import -window root "$OUT/3-export-dialog.png"
xdotool mousemove "$DIALOG_OK_X" "$DIALOG_OK_Y" click 1; sleep 4
xdotool key ctrl+7; sleep 2
import -window root "$OUT/4-exports.png"
xdotool key ctrl+5; sleep 2.5
import -window root "$OUT/5-editor.png"
python3 - <<'PY'
import sqlite3
c = sqlite3.connect('/tmp/home/.local/share/com.mestiaque.enrecorder/enrecorder.db')
print("recordings:", c.execute("select kind, title, duration_ms from recordings").fetchall())
print("exports:", c.execute("select status, output_path, error from export_jobs").fetchall())
PY
ls /tmp/home/.local/share/com.mestiaque.enrecorder/thumbnails 2>&1 | head
grep -E "ERROR|WARN" "$OUT/app.log" | grep -v "encoder\|clipboard" | tail -5
