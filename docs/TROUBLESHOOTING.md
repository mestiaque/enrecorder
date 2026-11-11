# Troubleshooting

**"FFmpeg not found"** — Ubuntu: `sudo apt install ffmpeg`. Windows: install FFmpeg and add it to
PATH, or choose `ffmpeg.exe` in Settings → Tools. ENRecorder looks in the custom path first, then
next to the program and in its data folder, then on PATH.

**A hardware encoder shows "Test failed"** — hover the red text to see FFmpeg's message. Common
reasons: the GPU driver lacks the media part (VA-API: `intel-media-va-driver` /
`mesa-va-drivers`; QSV: Intel's oneVPL/Media SDK runtime), the FFmpeg build was compiled without
it, or there is no such GPU (NVENC without NVIDIA). Software encoders always work as a fallback.
After installing a driver, press **Test again**. Results are cached per FFmpeg path and version.

**Encoder test takes long the first time** — each encoder runs a one-second sample; a device that
does not answer is stopped after 10 s and its other encoders are skipped. Later starts use the
cache.

**"Neither PipeWire nor PulseAudio answers"** — audio and (on Wayland) screen capture need
PipeWire: `systemctl --user status pipewire wireplumber`.

**Wayland** — screen capture always goes through your desktop's sharing dialog (xdg-desktop-portal);
make sure `xdg-desktop-portal` and the portal for your desktop (`-gnome`, `-kde`) are installed.

**Settings, library or logs** — Settings → About shows the folders. Settings → Logs has the log
and **Copy diagnostics** for bug reports.

**Reset** — Settings → General → Reset all settings (recordings are not touched).

**Recording does not start: "FFmpeg could not start recording: …"** — the message is FFmpeg's
own. With a hardware encoder, choose "Automatic" or a software encoder (libx264) and try again;
Settings → Logs shows the full FFmpeg command.

**The area selector covers the screen in black** — your desktop has no compositor (rare on
GNOME/KDE). The selection still works; the black is only the overlay's background.

**Ctrl+Shift+R does nothing** — another app may already use the shortcut (Settings → Logs shows
"shortcut … is not available"). Use the tray menu or the floating controls instead.

**Where are the parts of a failed recording?** — in a hidden folder `.enrecorder-<id>` inside
the recordings folder; each part is a playable MKV. Automatic recovery arrives in phase 7.

**Wayland: "screen sharing portal is not available"** — install `xdg-desktop-portal` and the
portal for your desktop (`xdg-desktop-portal-gnome`, `-kde` or `-wlr`).

**Wayland: "GStreamer's PipeWire support is missing"** — `sudo apt install gstreamer1.0-pipewire
gstreamer1.0-tools` (the .deb installs them).
