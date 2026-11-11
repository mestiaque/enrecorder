#!/usr/bin/env bash
# Headless end-to-end recording test (inside the build container): open the Video Recorder,
# press "Start recording", let it record the Xvfb screen (with a moving test pattern window),
# pause and resume with Ctrl+Shift+P, stop with Ctrl+Shift+R, then verify the file with
# FFprobe and the library row in SQLite.
set -uo pipefail
OUT="${1:-/src/target/smoke-record}"
BIN="${BIN:-/src/target/release/enrecorder}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run XDG_DATA_HOME=/tmp/home/.local/share XDG_CONFIG_HOME=/tmp/home/.config
rm -rf /tmp/home/.local/share/com.mestiaque.enrecorder /tmp/home/Videos/ENRecorder
mkdir -p "$XDG_RUNTIME_DIR" "$HOME/Videos" "$HOME/Pictures" && chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 &
XVFB=$!
export DISPLAY=:99 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
sleep 1
dbus-run-session -- "$BIN" >"$OUT/app.log" 2>&1 &
APP=$!
trap 'kill $APP $XVFB 2>/dev/null' EXIT
sleep 22  # the first start tests every encoder (≈15 s in the container)
X=80; Y=50
xdotool mousemove $((X + 640)) $((Y + 400)) click 1
xdotool key ctrl+2; sleep 2.5
import -window root "$OUT/1-video-page.png"
# "Start recording" button (position from the layout at 1280×800).
xdotool mousemove ${START_X:-$((X + 330))} ${START_Y:-$((Y + 445))} click 1
sleep 1.2; import -window root "$OUT/2-countdown.png"
sleep 4; import -window root "$OUT/3-recording.png"
# Floating controls sit at the top centre: pause/resume, then stop.
HUD_Y=111
xdotool mousemove 802 $HUD_Y click 1; sleep 2; import -window root "$OUT/4-paused.png"
xdotool mousemove 802 $HUD_Y click 1; sleep 2.5
xdotool mousemove 843 $HUD_Y click 1; sleep 6
import -window root "$OUT/5-done.png"
# Global shortcut check: start again with Ctrl+Shift+R, stop with Ctrl+Shift+R.
xdotool key ctrl+shift+r; sleep 6; xdotool key ctrl+shift+r; sleep 5
import -window root "$OUT/6-after-shortcut.png"
kill $APP; sleep 1
echo "--- app log ---"; grep -E "recording|segment|FFmpeg|ERROR|WARN" "$OUT/app.log" | grep -v "encoder " | tail -20
echo "--- files ---"; ls -la /tmp/home/Videos/ENRecorder 2>&1
for f in /tmp/home/Videos/ENRecorder/*.mp4; do
  [ -f "$f" ] && ffprobe -v error -show_entries format=duration:stream=codec_name,width,height -of compact "$f"
done
python3 - <<'PY'
import sqlite3
c = sqlite3.connect('/tmp/home/.local/share/com.mestiaque.enrecorder/enrecorder.db')
print("recordings:", c.execute("select kind, title, duration_ms, width, height, status from recordings").fetchall())
print("sessions:", c.execute("select state, error from capture_sessions").fetchall())
print("metadata:", c.execute("select video_codec, duration_ms from media_metadata").fetchall())
PY
