#!/usr/bin/env bash
# Wayland end to end (inside enrec-build): headless sway + PipeWire + WirePlumber +
# xdg-desktop-portal(-wlr, auto-selecting the only output) → ENRecorder's Wayland backend asks
# the portal for a screen cast and records it. Run with: dbus-run-session -- this-script.
set -uo pipefail
OUT="${1:-/src/target/wayland-e2e}"
rm -rf "$OUT" && mkdir -p "$OUT"
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/xdg-run-wl XDG_CONFIG_HOME=/tmp/home/.config
rm -rf "$XDG_RUNTIME_DIR" && mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME/xdg-desktop-portal-wlr" && chmod 700 "$XDG_RUNTIME_DIR"
printf '[screencast]\nchooser_type=none\noutput_name=HEADLESS-1\n' > "$XDG_CONFIG_HOME/xdg-desktop-portal-wlr/config"
printf 'output HEADLESS-1 resolution 1280x720\nexec foot -e top\n' > "$OUT/sway.conf"
export XDG_CURRENT_DESKTOP=sway XDG_SESSION_TYPE=wayland WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
pipewire >"$OUT/pipewire.log" 2>&1 &
sleep 1; wireplumber >"$OUT/wireplumber.log" 2>&1 &
sway -c "$OUT/sway.conf" >"$OUT/sway.log" 2>&1 &
for i in $(seq 1 50); do [ -S "$XDG_RUNTIME_DIR/wayland-1" ] && break; sleep 0.1; done
export WAYLAND_DISPLAY=wayland-1
/usr/libexec/xdg-desktop-portal-wlr -l DEBUG >"$OUT/portal-wlr.log" 2>&1 &
sleep 1; /usr/libexec/xdg-desktop-portal -v >"$OUT/portal.log" 2>&1 &
sleep 2
target/debug/examples/record_wayland "$OUT" 2>&1 | tee "$OUT/record.log" | grep -vE "^\[" 
for f in "$OUT"/*.mp4; do [ -f "$f" ] && ffprobe -v error -show_entries format=duration:stream=codec_name,width,height,avg_frame_rate -of compact "$f"; done
# A frame from the middle of the recording, to see that real screen content was captured.
for f in "$OUT"/*.mp4; do [ -f "$f" ] && ffmpeg -v error -y -ss 1.5 -i "$f" -frames:v 1 "$OUT/frame.png"; done
kill %1 %2 %3 %4 %5 2>/dev/null; true
