#!/usr/bin/env bash
# Downloads a static Windows FFmpeg build (BtbN/FFmpeg-Builds, GPL, release branch) and puts
# ffmpeg.exe, ffprobe.exe and its licence into src-tauri/ffmpeg-bin for the NSIS installer.
# The zip is checked against the checksums.sha256 published with the same release.
# Runs in Git Bash (CI: windows-latest) or on Linux/macOS. FFMPEG_ASSET picks another zip.
set -euo pipefail
cd "$(dirname "$0")/../.."
BASE="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest"
DEST="src-tauri/ffmpeg-bin"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

curl -fsSL --retry 3 -o "$WORK/checksums.sha256" "$BASE/checksums.sha256"
# Newest release branch ("ffmpeg-n7.1-latest-win64-gpl-7.1.zip"), not master and not "shared".
ASSET="${FFMPEG_ASSET:-$(grep -oE 'ffmpeg-n[0-9.]+-latest-win64-gpl-[0-9.]+\.zip' "$WORK/checksums.sha256" | sort -V | tail -1)}"
[ -n "$ASSET" ] || { echo "no Windows GPL release build listed in checksums.sha256" >&2; exit 1; }
echo "FFmpeg: $ASSET"
curl -fsSL --retry 3 -o "$WORK/$ASSET" "$BASE/$ASSET"
(cd "$WORK" && grep " \*\?$ASSET\$" checksums.sha256 | sed 's/ \*/  /' | sha256sum -c -)

unzip -q "$WORK/$ASSET" -d "$WORK/x"
DIR="$WORK/x/${ASSET%.zip}"
mkdir -p "$DEST"
cp "$DIR/bin/ffmpeg.exe" "$DIR/bin/ffprobe.exe" "$DEST/"
cp "$DIR/LICENSE.txt" "$DEST/FFMPEG-LICENSE.txt"
ls -l "$DEST"
