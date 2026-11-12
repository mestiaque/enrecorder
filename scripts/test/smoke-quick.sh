#!/usr/bin/env bash
# Quick capture (inside enrec-build, Xvfb): a test picture on the desktop, the quick-capture
# shortcut (Ctrl+Alt+A) freezes it, an area is dragged, an arrow, a blur and a text are drawn,
# Ctrl+S saves it to the library (checked in SQLite and with FFprobe); a second capture is copied
# with Enter (clipboard checked when xclip is installed).
set -uo pipefail
OUT="${1:-/src/target/smoke-quick}"
BIN="${BIN:-/src/target/release/enrecorder}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run XDG_DATA_HOME=/tmp/home/.local/share XDG_CONFIG_HOME=/tmp/home/.config
DATA=/tmp/home/.local/share/com.mestiaque.enrecorder
rm -rf "$DATA" /tmp/home/Pictures/ENRecorder
mkdir -p "$XDG_RUNTIME_DIR" "$HOME/Videos" "$HOME/Pictures" && chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 &
export DISPLAY=:99 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
sleep 1
convert -size 900x560 gradient:'#1e3a8a-#9333ea' -font DejaVu-Sans -pointsize 54 -fill white -annotate +60+140 'Quick capture test' \
  -pointsize 30 -annotate +60+220 'secret: 1234-5678' "$OUT/picture.png"
display -geometry +260+170 "$OUT/picture.png" &
sleep 1
dbus-run-session -- "$BIN" >"$OUT/app.log" 2>&1 &
APP=$!
trap 'kill $APP 2>/dev/null; pkill -x enrecorder; pkill -x display; pkill -x Xvfb' EXIT
sleep 22
drag() { xdotool mousemove "$1" "$2" mousedown 1; sleep 0.1; xdotool mousemove "$3" "$4"; sleep 0.1; xdotool mousemove "$5" "$6"; sleep 0.1; xdotool mouseup 1; sleep 0.4; }
xdotool key ctrl+alt+a; sleep 2.5
import -window root "$OUT/1-frozen.png"
drag 300 220 700 500 1100 700          # the area
import -window root "$OUT/2-area.png"
drag 380 450 500 420 640 380           # arrow (default tool)
xdotool key b; drag 330 370 500 390 740 410   # blur over the "secret" line
xdotool key t; xdotool mousemove 700 600 click 1; sleep 0.5; xdotool type --delay 40 "Hello"; xdotool key Return; sleep 0.5
import -window root "$OUT/3-annotated.png"
xdotool key ctrl+s; sleep 3
import -window root "$OUT/4-after-save.png"
echo "--- saved ---"
python3 - <<'PY'
import sqlite3
c = sqlite3.connect('/tmp/home/.local/share/com.mestiaque.enrecorder/enrecorder.db')
for r in c.execute("select title, path, width, height, length(annotations) from screenshots"):
    print("screenshot:", r)
PY
for f in /tmp/home/Pictures/ENRecorder/*.png; do ffprobe -v error -show_entries stream=width,height -of compact "$f"; done
echo "originals: $(ls /tmp/home/Pictures/ENRecorder/.enrecorder-originals 2>/dev/null | wc -l)"
echo "temporary quick files left: $(ls "$DATA/tmp" 2>/dev/null | grep -c quick)"
# Second capture: copy with Enter.
xdotool key ctrl+alt+a; sleep 2.5
drag 400 300 500 350 640 460
xdotool key Return; sleep 1.5
if command -v xclip >/dev/null; then
  xclip -selection clipboard -t image/png -o > "$OUT/clipboard.png" 2>/dev/null && ffprobe -v error -show_entries stream=width,height -of compact "$OUT/clipboard.png"
fi
xdotool mousemove 720 450 click 1; xdotool key ctrl+4; sleep 2.5; import -window root "$OUT/5-studio.png"
pgrep -f "view=quick" >/dev/null && echo "overlay still open" || echo "overlay closed"
grep -E "quick|ERROR|WARN" "$OUT/app.log" | grep -vE "encoder|wayland|config file|already registered|not available" | tail -6
