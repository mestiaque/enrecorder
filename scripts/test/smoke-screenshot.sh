#!/usr/bin/env bash
# Headless Screenshot Studio test (Xvfb/X11): Ctrl+4, take a full-screen screenshot of a test
# pattern, draw a rectangle and a blur in the editor, save as WebP (Ctrl+S after choosing the
# format), check the file and the library row.  STEP=shot → only screenshot the page.
set -uo pipefail
OUT="${1:-/src/target/smoke-screenshot}"
BIN="${BIN:-/src/target/debug/enrecorder}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run XDG_DATA_HOME=/tmp/home/.local/share XDG_CONFIG_HOME=/tmp/home/.config
rm -rf /tmp/home/.local/share/com.mestiaque.enrecorder /tmp/home/Pictures/ENRecorder
mkdir -p "$XDG_RUNTIME_DIR" "$HOME/Videos" "$HOME/Pictures" && chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 &
export DISPLAY=:99 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
sleep 1
# Something recognisable on the screen behind the app.
# (drawn on the root window: a player window would take the keyboard focus)
ffmpeg -hide_banner -loglevel error -f lavfi -i "testsrc2=size=1440x900" -frames:v 1 -y /tmp/pattern.png
display -window root /tmp/pattern.png >/dev/null 2>&1 &
dbus-run-session -- "$BIN" >"$OUT/app.log" 2>&1 &
APP=$!
trap 'kill $APP 2>/dev/null; pkill display; pkill Xvfb' EXIT
sleep 22
xdotool mousemove 720 450 click 1
xdotool key ctrl+4; sleep 2.5
import -window root "$OUT/1-page.png"
[ "${STEP:-}" = shot ] && exit 0
xdotool mousemove "$SHOT_X" "$SHOT_Y" click 1; sleep 4
import -window root "$OUT/2-editor.png"
xdotool key r; xdotool mousemove 500 300 mousedown 1 mousemove 600 380 mousemove 700 450 mouseup 1; sleep 0.5
xdotool key b; xdotool mousemove 800 300 mousedown 1 mousemove 900 380 mousemove 1000 450 mouseup 1; sleep 0.5
xdotool key n; xdotool mousemove 450 600 click 1; sleep 0.5
import -window root "$OUT/3-annotated.png"
xdotool key ctrl+s; sleep 3
import -window root "$OUT/4-saved.png"
echo "--- files ---"; ls -la /tmp/home/Pictures/ENRecorder /tmp/home/Pictures/ENRecorder/.enrecorder-originals 2>&1
for f in /tmp/home/Pictures/ENRecorder/*.*; do ffprobe -v error -show_entries stream=codec_name,width,height -of compact "$f"; done
python3 - <<'PY'
import sqlite3
c = sqlite3.connect('/tmp/home/.local/share/com.mestiaque.enrecorder/enrecorder.db')
for r in c.execute("select title, format, width, height, size_bytes, length(annotations) from screenshots"): print("screenshot:", r)
PY
grep -E "ERROR|WARN" "$OUT/app.log" | grep -v "encoder\|clipboard" | tail -5
