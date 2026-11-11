#!/usr/bin/env bash
# Area recording (inside the build container): Area tab → "Select an area…" → drag on the
# overlay → "Record this area" → record 3 s with Ctrl+Shift+R → verify the size with FFprobe.
set -uo pipefail
OUT="${1:-/src/target/smoke-region}"
BIN="${BIN:-/src/target/release/enrecorder}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run XDG_DATA_HOME=/tmp/home/.local/share XDG_CONFIG_HOME=/tmp/home/.config
rm -rf /tmp/home/Videos/ENRecorder
mkdir -p "$XDG_RUNTIME_DIR" "$HOME/Videos" && chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 &
XVFB=$!
export DISPLAY=:99 XDG_SESSION_TYPE=x11
sleep 1
dbus-run-session -- "$BIN" >"$OUT/app.log" 2>&1 &
APP=$!
trap 'kill $APP $XVFB 2>/dev/null' EXIT
sleep 8   # encoder results are cached from the previous test run
xdotool mousemove 720 450 click 1; xdotool key ctrl+2; sleep 2
xdotool mousemove 533 268 click 1; sleep 1.5          # "Area" tab
import -window root "$OUT/1-area-tab.png"
xdotool mousemove ${SEL_X:-420} ${SEL_Y:-318} click 1; sleep 3   # "Select an area…"
import -window root "$OUT/2-overlay.png"
xdotool mousemove 300 300 mousedown 1; sleep 0.2
xdotool mousemove 500 450; sleep 0.2; xdotool mousemove 701 601; sleep 0.2; xdotool mouseup 1; sleep 1
import -window root "$OUT/3-drawn.png"
xdotool mousemove 350 625 click 1; sleep 2              # "Record this area"
import -window root "$OUT/4-selected.png"
xdotool key ctrl+shift+r; sleep 7; xdotool key ctrl+shift+r; sleep 5
kill $APP; sleep 1
grep -E "segment 1|completed|failed|ERROR" "$OUT/app.log" | sed 's/-vf.*//' | tail -5
for f in /tmp/home/Videos/ENRecorder/*.mp4; do [ -f "$f" ] && ffprobe -v error -show_entries format=duration:stream=width,height -of compact "$f"; done
