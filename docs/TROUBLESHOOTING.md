# Troubleshooting

**"FFmpeg not found"** — Ubuntu: `sudo apt install ffmpeg`. Windows: install FFmpeg and add it to
PATH, or choose `ffmpeg.exe` in Settings → Tools. ENRecorder looks in the custom path first, then
next to the program and in its data folder, then on PATH.

**A hardware encoder shows "Not available"** — normal: that graphics hardware (NVIDIA, AMD,
Quick Sync) is not in this computer, or its driver cannot encode that format. Nothing has to be
installed; ENRecorder uses the best encoder that passed its test (graphics card first, otherwise
the processor). Hover "Not available" for the reason and FFmpeg's message. To try more GPU
formats on Intel, the non-free media driver (`intel-media-va-driver-non-free`) sometimes adds
H.265; press **Test again** after installing a driver. Results are cached per FFmpeg path and
version.

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
"shortcut … is not available"). Choose another combination in Settings → Shortcuts, or use the
tray menu or the floating controls.

**Where are the parts of a failed recording?** — in a hidden folder `.enrecorder-<id>` inside
the recordings folder; each part is a playable MKV. Home offers **Recover** for them after a
crash or power loss.

**Wayland: "screen sharing portal is not available"** — install `xdg-desktop-portal` and the
portal for your desktop (`xdg-desktop-portal-gnome`, `-kde` or `-wlr`).

**Wayland: "GStreamer's PipeWire support is missing"** — `sudo apt install gstreamer1.0-pipewire
gstreamer1.0-tools` (the .deb installs them).

## Transcription

ENRecorder does not include a speech engine. To use **Transcribe** (recording details):

1. Install whisper.cpp so that `whisper-cli` is on PATH, or build it
   (`cmake -B build && cmake --build build --target whisper-cli`) and choose `build/bin/whisper-cli`
   in Settings → Tools → Transcription. On Windows, choose `whisper-cli.exe` from a whisper.cpp
   release.
2. Download a model (`ggml-base.bin` is a good start; `.en` models understand English only) from
   the whisper.cpp model page and put it in the **Models folder** (button in Settings → Tools),
   or choose the file.

The .srt is saved next to the recording; video players load it automatically. "Not set up" in
Settings means the program or the model was not found.

**No click highlights or key overlay in the export** — "Remember clicks" / "Remember key presses"
must be on *before* recording (Recording settings). They work on X11 and Windows; Wayland does not
let apps watch the pointer or keyboard, so the options are hidden there.

**Echo cancellation does nothing** — it needs PipeWire's or PulseAudio's
`module-echo-cancel` (`pactl load-module module-echo-cancel` must work). Linux only.

**A scheduled recording did not start** — ENRecorder must be running (the tray is enough) at the
start time; a notification explains why it could not start.

**Quick capture (Ctrl+Alt+A) does nothing** — another program may hold the combination; choose
another one in Settings → Shortcuts (or use the tray menu or the button in the Screenshot Studio).
On Wayland the desktop asks once whether ENRecorder may take screenshots; if that was refused,
allow it again in the desktop's privacy settings.
