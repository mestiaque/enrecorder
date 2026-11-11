#!/usr/bin/env bash
# Feature walk-through (inside enrec-build, Xvfb): settings pages (Screenshots, Shortcuts,
# Devices, Performance, Privacy), a screen recording with "Remember clicks" and clicks during
# the recording (pointer track saved), the library in grid view with the filter panel, and the
# Media Editor on a video. Settings are seeded in SQLite before the second start.
set -uo pipefail
OUT="${1:-/src/target/smoke-features}"
BIN="${BIN:-/src/target/release/enrecorder}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run XDG_DATA_HOME=/tmp/home/.local/share XDG_CONFIG_HOME=/tmp/home/.config
DATA=/tmp/home/.local/share/com.mestiaque.enrecorder
rm -rf "$DATA" /tmp/home/Videos/ENRecorder
mkdir -p "$XDG_RUNTIME_DIR" "$HOME/Videos" "$HOME/Pictures" && chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 &
export DISPLAY=:99 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
sleep 1
start_app() { dbus-run-session -- "$BIN" >>"$OUT/app.log" 2>&1 & APP=$!; }
start_app; sleep 22; kill $APP; pkill -x enrecorder; sleep 3
python3 - <<'PY'
import sqlite3, json
c = sqlite3.connect('/tmp/home/.local/share/com.mestiaque.enrecorder/enrecorder.db')
for k, v in {"recTrackClicks": True, "recTrackKeys": True, "recCountdown": 0, "recShowHud": False, "recSplitMinutes": 0}.items():
    c.execute("INSERT INTO settings(key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value", (k, json.dumps(v)))
c.commit()
PY
start_app
trap 'kill $APP 2>/dev/null; pkill -x enrecorder; pkill -x Xvfb' EXIT
sleep 8
xdotool mousemove 720 450 click 1
xdotool key ctrl+comma; sleep 1.5
# Settings sections by position in the dialog's left column (11 sections, 34 px apart).
for i in 2 4 5 6 7 8; do
  xdotool mousemove 340 $((202 + 34 * i)) click 1; sleep 1.5
  import -window root "$OUT/settings-$i.png"
done
xdotool key Escape; sleep 1
xdotool key ctrl+2; sleep 2.5
import -window root "$OUT/video-page.png"
xdotool key ctrl+shift+r; sleep 3
# Clicks on the (empty) desktop while recording: the window is hidden.
# Held like a person would (a polling tracker cannot see a zero-length synthetic click).
for p in "200 200" "600 400" "1000 700"; do xdotool mousemove $p mousedown 1; sleep 0.12; xdotool mouseup 1; sleep 0.6; done
xdotool keydown ctrl+alt+t; sleep 0.15; xdotool keyup ctrl+alt+t; sleep 0.3
sleep 1; xdotool key ctrl+shift+r; sleep 6
import -window root "$OUT/after-recording.png"
echo "--- tracks ---"; ls -la "$DATA/tracks" 2>&1
python3 - <<'PY'
import json, glob
for f in glob.glob('/tmp/home/.local/share/com.mestiaque.enrecorder/tracks/*.json'):
    t = json.load(open(f))
    print("track:", len(t["events"]), "events,", sum(1 for e in t["events"] if e.get("click")), "clicks,", [e["key"] for e in t["events"] if e.get("key")], f"{t['width']}x{t['height']}")
PY
xdotool key ctrl+6; sleep 2.5
xdotool mousemove 1223 209 click 1; sleep 1.5    # grid view
xdotool mousemove 995 209 click 1; sleep 1.5     # filters
import -window root "$OUT/library-grid-filters.png"
xdotool key Escape; sleep 0.5
xdotool key ctrl+5; sleep 3
import -window root "$OUT/editor-video.png"
grep -E "ERROR|WARN|pointer track|global shortcut" "$OUT/app.log" | grep -vE "encoder|clipboard|wayland|config file" | tail -8
