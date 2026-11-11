#!/usr/bin/env bash
# Headless smoke test (inside the build container, see BUILD.md): start the release binary
# under Xvfb, visit every page with the keyboard shortcuts, open Settings, take screenshots,
# then check the database and the log.
set -uo pipefail
OUT="${1:-/src/target/smoke}"
BIN="${BIN:-/src/target/release/enrecorder}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run XDG_DATA_HOME=/tmp/home/.local/share XDG_CONFIG_HOME=/tmp/home/.config
rm -rf /tmp/home/.local/share/com.mestiaque.enrecorder
mkdir -p "$XDG_RUNTIME_DIR" "$HOME/Videos" "$HOME/Pictures" && chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 &
XVFB=$!
trap 'kill $APP $XVFB 2>/dev/null' EXIT
export DISPLAY=:99 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
sleep 1
dbus-run-session -- "$BIN" >"$OUT/app.log" 2>&1 &
APP=$!
sleep 8
# Without a window manager the frameless window sits centred; find it, or assume the centre.
X=80; Y=50; WIDTH=1280; HEIGHT=800
for w in $(xdotool search --name ENRecorder 2>/dev/null); do
  g=$(xdotool getwindowgeometry --shell "$w" 2>/dev/null) || continue
  eval "$g"
  [ "${WIDTH:-0}" -gt 600 ] && { WID=$w; break; }
done
echo "window: ${WID:-not found} at $X,$Y ${WIDTH}x$HEIGHT"
[ -n "${WID:-}" ] && xdotool windowfocus "$WID" 2>/dev/null
xdotool mousemove $((X + WIDTH / 2)) $((Y + HEIGHT / 2)) click 1
sleep 1
import -window root "$OUT/1-home.png"
n=2
for page in video audio screenshot editor recordings exports; do
  xdotool key "ctrl+$n"; sleep 1.5
  import -window root "$OUT/$n-$page.png"
  n=$((n + 1))
done
xdotool key ctrl+1; sleep 1
xdotool key ctrl+comma; sleep 1.5
import -window root "$OUT/s1-general.png"
# Settings sections by their position in the dialog's left column (dialog is centred).
for i in 1 2 3 5; do
  xdotool mousemove $((X + WIDTH / 2 - 380)) $((Y + HEIGHT / 2 - 230 + 32 * i)) click 1; sleep 1.2
  import -window root "$OUT/s$((i + 1))-section.png"
done
# Appearance → Light theme card.
xdotool mousemove $((X + WIDTH / 2 - 380)) $((Y + HEIGHT / 2 - 230 + 32 * 3)) click 1; sleep 1
xdotool mousemove $((X + WIDTH / 2 - 10)) $((Y + HEIGHT / 2 - 200)) click 1; sleep 1
xdotool key Escape; sleep 1
import -window root "$OUT/light-home.png"
kill $APP; sleep 1
echo "--- app log ---"; tail -25 "$OUT/app.log"
echo "--- database ---"
python3 - <<'PY'
import sqlite3, glob
db = glob.glob('/tmp/home/.local/share/com.mestiaque.enrecorder/enrecorder.db')[0]
c = sqlite3.connect(db)
print("tables:", sorted(r[0] for r in c.execute("select name from sqlite_master where type='table'")))
print("migrations:", c.execute("select version, name from schema_migrations").fetchall())
print("settings:", c.execute("select key, value from settings").fetchall())
print("foreign_keys pragma needs the app connection; integrity:", c.execute("pragma integrity_check").fetchone()[0])
PY
ls -la /tmp/home/.local/share/com.mestiaque.enrecorder/ /tmp/home/.local/share/com.mestiaque.enrecorder/cache 2>&1 | tail -12
