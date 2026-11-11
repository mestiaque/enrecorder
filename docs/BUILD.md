# Building ENRecorder

## Ubuntu (22.04 / 24.04)

### Option A — Docker (no development packages on the host)

The image has Rust, Node 20, pnpm, WebKitGTK 4.1 headers, FFmpeg and Xvfb for headless tests.

```bash
docker build -t endl-build scripts/docker          # once
npx -y pnpm@9.15.0 install                         # frontend packages (host or container)
scripts/docker/run.sh "pnpm tauri build --bundles deb"
# → target/release/bundle/deb/ENRecorder_0.2.0_amd64.deb
scripts/docker/run.sh "scripts/test/smoke-ui.sh"    # headless UI test with screenshots in target/smoke
```

### Option B — native

```bash
sudo apt install build-essential curl pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
  librsvg2-dev libayatana-appindicator3-dev libssl-dev libxdo-dev ffmpeg
# Rust (rustup) and Node.js 20+, then:
npx -y pnpm@9.15.0 install
npx -y pnpm@9.15.0 tauri build --bundles deb       # or: tauri dev
```

Later phases add capture dependencies (documented when they land): PipeWire and
xdg-desktop-portal development headers for Wayland, XCB/XShm for X11.

### Tests

```bash
cargo test -p enrec-core                 # Rust: database, settings, FFmpeg, storage, system, state machine
cargo clippy -p enrec-core --all-targets -- -D warnings
npx -y pnpm@9.15.0 typecheck             # strict TypeScript
npx -y pnpm@9.15.0 test                  # Vitest: readiness, formatting, navigation store
```

The FFmpeg and session tests run real encodes when FFmpeg is installed; the X11 test uses the
real X server when `DISPLAY` is set.

Hardware checks on a real X11 desktop (records the screen for 6 s into the given folder):

```bash
cargo run -p enrec-core --example x11_sources
cargo run -p enrec-core --example record_desktop -- /tmp/rec [encoder]
```

Headless UI tests (Docker): `scripts/test/smoke-ui.sh`, `scripts/test/smoke-record.sh`,
`scripts/test/smoke-region.sh` (screenshots in `target/smoke*`).

### Runtime dependencies

`ffmpeg` (declared in the .deb; the AppImage uses the system one). WebKitGTK 4.1 and GTK 3 come with Ubuntu 22.04+.

## Windows (10 / 11)

Built by CI (`.github/workflows/build.yml`, `windows-latest`). Locally, with Rust (MSVC),
Node.js 20+, Git Bash and the WebView2 runtime:

```powershell
bash scripts/release/fetch-ffmpeg-windows.sh   # FFmpeg + FFprobe into src-tauri\ffmpeg-bin (SHA-256 checked)
npx -y pnpm@9.15.0 install
npx -y pnpm@9.15.0 tauri build --bundles nsis  # target\release\bundle\nsis\ENRecorder_<ver>_x64-setup.exe
```

`src-tauri/tauri.windows.conf.json` puts `ffmpeg.exe`, `ffprobe.exe` and `FFMPEG-LICENSE.txt`
next to `enrecorder.exe`, where ENRecorder looks first (then PATH, then Settings → Tools).

## Releases

CI runs the tests on every push. A tag `v<version>` (matching `tauri.conf.json`) also builds the
.deb, the AppImage and the Windows installer and publishes them as a GitHub release; the in-app
updater picks them up by name (`ENRecorder_<ver>_amd64.deb`, `_amd64.AppImage`, `_x64-setup.exe`).
