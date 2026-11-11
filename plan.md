# MASTER DEVELOPMENT PROMPT — CAPTURE STUDIO

## 1. ROLE & RESPONSIBILITY

Act as a Senior Cross-Platform Desktop Application Architect, Rust Engineer, Tauri 2 Developer, React/TypeScript Engineer, Multimedia Systems Engineer, FFmpeg Specialist, and Premium UI/UX Designer.

Your mission is to design and develop a production-quality desktop application named **Capture Studio**.

This is a complete video recording, audio recording, screenshot capture, and media processing application for Windows and Ubuntu Linux.

Do not build a simple demonstration, static dashboard, or frontend-only prototype. Build a real, functional application with native operating-system integration.

You are responsible for:
- Architecture and technical decisions.
- Frontend implementation.
- Rust backend implementation.
- Native capture integrations.
- FFmpeg integration.
- Database and file management.
- Testing and debugging.
- Cross-platform compatibility.
- Production packaging and documentation.

Think like an experienced software engineering team. Prioritize correctness, maintainability, performance, reliability, and excellent user experience.

---

## 2. PRODUCT IDENTITY

**Application Name:** Capture Studio

**Tagline:** One Studio. Every Capture.

**Product Category:** Professional Desktop Capture & Media Studio.

### Supported Platforms

Primary:
- Windows 10 and Windows 11.
- Ubuntu 22.04 LTS.
- Ubuntu 24.04 LTS.

Linux display systems:
- X11.
- Wayland.

Design the architecture so future Linux distributions can be supported without rewriting the application.

### Product Philosophy

1. Premium user experience.
2. Lightweight desktop application.
3. Native OS integration.
4. Local-first architecture.
5. Privacy-focused operation.
6. No mandatory account.
7. No mandatory cloud service.
8. No watermark.
9. No unnecessary background services.
10. No fake or non-functional features.

---

## 3. FINAL TECHNOLOGY STACK

Use the following stack unless a technically justified compatibility issue requires an alternative.

### Desktop
- Tauri 2.
- Rust.
- WebView-based frontend.

### Frontend
- React.
- TypeScript with strict mode.
- Vite.
- Tailwind CSS.
- shadcn/ui.
- Radix UI.
- Zustand.
- TanStack Query.
- React Hook Form.
- Zod.
- Lucide Icons.
- Framer Motion.

### Backend
- Rust.
- Tokio where appropriate.
- Tauri commands and events.
- Strongly typed application state.
- Platform-specific native modules.

### Media
- FFmpeg.
- FFprobe.
- Native capture APIs.
- Hardware-accelerated encoding when available.

### Database
- SQLite.
- SQLx or an appropriate maintained Rust SQLite library.
- Versioned migrations.

### Audio
- CPAL for supported cross-platform audio input/output.
- WASAPI for Windows-specific audio capture and loopback.
- PipeWire for Linux audio and system audio capture.
- Native processing where required.

### Image
- Rust image processing libraries.
- PNG, JPEG, WebP support.

Do not introduce unnecessary frameworks or duplicate dependencies.

---

## 4. APPLICATION MODULES

Build the application around the following main modules.

1. Dashboard.
2. Video Recorder.
3. Audio Recorder.
4. Screenshot Studio.
5. Media Editor.
6. Recordings Library.
7. Export Center.
8. Settings.
9. System Tray.
10. Global Shortcuts.
11. Device Manager.
12. Diagnostics and Recovery.

Every module must have a clearly defined responsibility and communicate through documented interfaces.

---

## 5. PREMIUM UI/UX REQUIREMENTS

Create a premium, modern, professional desktop application.

### Design Direction

Use a sophisticated dark studio aesthetic.

Visual characteristics:
- Deep charcoal background.
- Subtle layered surfaces.
- Soft borders.
- Elegant blue/violet accent colors.
- Refined typography.
- Consistent spacing.
- Subtle shadows.
- Smooth transitions.
- Minimal visual clutter.
- Professional iconography.

Do not make the UI look like a generic admin dashboard.

Avoid:
- Excessive gradients.
- Excessive glassmorphism.
- Huge decorative headings.
- Too many cards.
- Unnecessary animations.
- Overcrowded toolbars.
- Inconsistent icon sizes.

### Theme Support

Implement:
- Dark theme.
- Light theme.
- System theme.

Persist the user's preference.

### Main Navigation

Create a clean sidebar with:

- Home
- Video Recorder
- Audio Recorder
- Screenshot Studio
- Media Editor
- Recordings
- Export Center
- Settings

Provide an appropriate active navigation state.

### Dashboard

Show:
- Quick recording actions.
- Recent recordings.
- Recent screenshots.
- Storage usage.
- Available disk space.
- Recording statistics.
- Quick access to the latest media.
- System readiness information.

### Interaction Quality

- Keyboard accessible controls.
- Consistent tooltips.
- Loading states.
- Empty states.
- Error states.
- Confirmation dialogs.
- Toast notifications.
- Responsive layout for different desktop window sizes.

Build reusable UI components instead of repeating markup.

---

## 6. VIDEO RECORDER ENGINE

Build a real native screen recording engine.

### Capture Sources

Support:
- Entire desktop.
- Individual monitor.
- Application window.
- Custom region.
- Screen + webcam.
- Webcam only.

Allow the user to select the source before recording.

### Windows Implementation

Use Windows-native technologies:

- Windows.Graphics.Capture for screen and window capture.
- WASAPI for audio.
- WASAPI loopback for system audio.
- Media Foundation or another suitable native webcam API.

Handle:
- Display selection.
- Window selection.
- Capture item resizing.
- Device disconnection.
- Permission and picker cancellation.
- Graphics device loss.
- Multi-monitor configurations.

Use a Rust-compatible Windows API implementation.

Do not substitute a browser-based screen capture API for the native recording engine.

### Linux Implementation

Support both X11 and Wayland.

Wayland:
- XDG Desktop Portal.
- ScreenCast portal.
- PipeWire streams.
- User-controlled source selection.
- Portal permission handling.

X11:
- Dedicated X11 capture backend.
- Monitor and region selection.

Handle:
- GNOME.
- KDE Plasma.
- Portal cancellation.
- PipeWire connection failure.
- Display changes.
- Mixed display resolutions.
- Unsupported compositor capabilities.

Detect the active display server automatically.

Do not assume X11 support means Wayland support.

### Video Settings

Resolution:
- Native.
- 720p.
- 1080p.
- 1440p.
- 4K.

Frame rates:
- 24 FPS.
- 30 FPS.
- 60 FPS.
- 120 FPS where supported.

Formats:
- MP4.
- MKV.
- WebM.

Codecs:
- H.264.
- H.265.
- AV1 where supported.

Encoders:
- Software encoding.
- NVIDIA NVENC.
- AMD AMF.
- Intel Quick Sync.
- Other FFmpeg-supported hardware encoders.

Implement runtime capability detection.

Never show an encoder as available unless it has been verified.

### Recording Controls

Implement:
- Start.
- Pause.
- Resume.
- Stop.
- Recording timer.
- Recording indicator.
- Countdown.
- Automatic stop.
- Scheduled recording.
- File splitting.
- Output path selection.
- Automatic naming.
- Disk space monitoring.

### Cursor Effects

Support:
- Show/hide cursor.
- Cursor highlight.
- Mouse click animation.
- Optional keystroke visualization.

Clearly distinguish between captured cursor effects and effects applied during post-processing.

### Webcam Overlay

Implement:
- Webcam device selection.
- Live preview.
- Position adjustment.
- Size adjustment.
- Circular and rectangular layouts.
- Rounded corners.
- Border customization.
- Shadow.
- Optional background.

Ensure webcam and screen timestamps remain synchronized.

### Recording HUD

Provide a lightweight floating recording toolbar.

Controls:
- Pause/resume.
- Stop.
- Elapsed time.
- Audio levels.
- Recording status.

Ensure the toolbar does not accidentally appear in the captured output unless the user explicitly requests it.

---

## 7. AUDIO RECORDER ENGINE

Build a professional audio recording system.

### Sources

Support:
- Microphone.
- System audio.
- Microphone + system audio.
- Multiple inputs where supported.

### Device Manager

Show:
- Device name.
- Device type.
- Input availability.
- Sample rates.
- Channel count.
- Default device.
- Current connection state.

Allow the user to select devices.

### Audio Configuration

Sample rates:
- 44.1 kHz.
- 48 kHz.
- 96 kHz where supported.

Formats:
- WAV.
- MP3.
- FLAC.
- AAC.
- OGG.

Support appropriate sample formats and channel layouts.

### Live Audio UI

Implement:
- Waveform.
- Input level meter.
- Peak meter.
- RMS meter.
- Recording timer.
- Device status.
- Clipping warning.

### Audio Processing

Provide optional:
- Noise suppression.
- Noise gate.
- Echo cancellation.
- Normalization.
- Gain adjustment.
- Silence detection.

Do not claim AI noise removal unless a real processing model is integrated.

### Multi-track Recording

When recording microphone and system audio simultaneously, preserve separate tracks.

Provide optional mixed output.

Never silently merge separate sources when the user selected separate tracks.

### Synchronization

Use reliable timestamps and appropriate buffering.

Handle:
- Different sample rates.
- Audio device changes.
- Buffer underruns.
- Input disconnection.
- Long recording sessions.

---

## 8. SCREENSHOT STUDIO

Build a complete screenshot capture and annotation tool.

### Capture Modes

- Full screen.
- Individual monitor.
- Application window.
- Custom region.
- Delayed screenshot.
- Multi-monitor screenshot.
- Scrolling capture where technically supported.

### Annotation Tools

Implement:
- Arrow.
- Line.
- Rectangle.
- Circle.
- Freehand drawing.
- Text.
- Numbered steps.
- Highlight.
- Blur.
- Pixelate.
- Crop.
- Resize.
- Rotate.
- Border.
- Shadow.
- Color picker.

### Editor Experience

Provide:
- Undo.
- Redo.
- Zoom.
- Pan.
- Selection.
- Layer ordering.
- Delete selected object.
- Copy/paste.
- Save.
- Export.

Annotations should remain editable until the user exports or flattens the image.

### Export Formats

- PNG.
- JPEG.
- WebP.

Support:
- Clipboard copy.
- Save dialog.
- Configurable default save path.
- Automatic file naming.

Scrolling capture must use a content-aware or otherwise verified implementation. Do not pretend arbitrary application windows can always be scrolled and captured reliably.

---

## 9. MEDIA LIBRARY

Build a professional local media library.

### Views

- Grid view.
- List view.

### Features

- Search.
- Sort.
- Filter.
- Favorites.
- Tags.
- Rename.
- Delete.
- Bulk delete.
- Open file location.
- Quick preview.
- Metadata inspection.
- Reprocess.
- Convert.
- Export.

### Filters

- Video.
- Audio.
- Screenshot.
- Date.
- Duration.
- File size.
- Resolution.
- Favorites.

### Media Metadata

Use FFprobe to inspect:
- Codec.
- Container.
- Resolution.
- Frame rate.
- Duration.
- Audio streams.
- Video streams.
- File size.
- Bitrate.

Generate thumbnails asynchronously.

Do not block the UI during media indexing.

Handle missing files and external file movement gracefully.

---

## 10. MEDIA EDITOR

Build a lightweight but functional media editor.

### Video Editing

- Preview player.
- Timeline.
- Trim start/end.
- Split.
- Crop.
- Resize.
- Rotate.
- Speed adjustment.
- Volume adjustment.
- Audio extraction.
- Thumbnail extraction.

### Audio Editing

- Waveform.
- Trim.
- Split.
- Fade in/out.
- Normalize.
- Volume adjustment.
- Silence detection.
- Optional noise processing.

### Export

Provide:
- Output format.
- Resolution.
- Frame rate.
- Video codec.
- Audio codec.
- Bitrate.
- Quality.
- Output location.
- Estimated file size where feasible.

Start with non-destructive editing where possible.

Never overwrite the original recording by default.

---

## 11. EXPORT JOB MANAGER

Implement a centralized export queue.

Features:
- Multiple export jobs.
- Job progress.
- Job status.
- Cancel.
- Retry.
- Error details.
- Output file location.
- Completed job history.

Use a safe job lifecycle:

QUEUED → INITIALIZING → RUNNING → FINALIZING → COMPLETED

Support FAILED and CANCELLED states.

Manage FFmpeg processes carefully.

Prevent duplicate jobs and clean up temporary resources.

---

## 12. FFmpeg INTEGRATION

FFmpeg is the central media processing engine.

Implement a dedicated Rust media service.

Responsibilities:
- Locate FFmpeg.
- Verify executable.
- Detect version.
- Inspect encoder capabilities.
- Inspect decoder capabilities.
- Build safe arguments.
- Start processes.
- Monitor progress.
- Parse stderr.
- Cancel processes.
- Handle errors.
- Verify output files.

Use FFprobe for metadata extraction.

### Important Rules

1. Never execute arbitrary commands from frontend input.
2. Never concatenate untrusted strings into shell commands.
3. Prefer direct process argument arrays.
4. Validate all paths.
5. Restrict executable access.
6. Handle process termination.
7. Capture diagnostic logs.
8. Verify generated output.
9. Never assume all FFmpeg builds support the same codecs.
10. Respect FFmpeg licensing and redistribution obligations.

Implement a clear FFmpeg installation and bundling strategy for both Windows and Ubuntu.

---

## 13. DATABASE DESIGN

Use SQLite.

Create migrations for:

- recordings
- screenshots
- media_metadata
- capture_sessions
- export_jobs
- devices
- settings
- shortcuts
- tags
- media_tags
- favorites

Requirements:
- UUID primary keys.
- Foreign key constraints.
- Indexes.
- Transactions.
- Migration versioning.
- Error handling.
- Safe database initialization.
- Recovery from interrupted writes.

Store media files separately from database records.

Do not store large video/audio files as database blobs.

---

## 14. RUST ARCHITECTURE

Maintain a modular architecture.

Suggested modules:

src/
- app/
- commands/
- core/
- capture/
- audio/
- webcam/
- screenshot/
- media/
- database/
- storage/
- system/
- utils/

Platform implementations:

capture/windows/
capture/linux/

audio/windows/
audio/linux/

Create common traits and typed interfaces for platform-specific functionality.

Do not mix Windows APIs and Linux APIs inside one giant implementation.

### Core services

Implement:
- AppState.
- CaptureSessionManager.
- AudioSessionManager.
- DeviceManager.
- MediaService.
- FFmpegService.
- ScreenshotService.
- ExportJobManager.
- StorageService.
- SettingsService.
- RecoveryService.

Use thread-safe shared state.

Avoid blocking the Tauri main thread.

Use asynchronous operations where appropriate.

Use bounded queues for high-volume frame and audio data.

Define backpressure and frame-dropping policies explicitly.

---

## 15. TAURI COMMUNICATION

Use typed Tauri commands for user actions.

Use Tauri events or channels for continuous status and progress updates.

Example command groups:

Video:
- list_capture_sources
- get_video_capabilities
- start_video_recording
- pause_video_recording
- resume_video_recording
- stop_video_recording

Audio:
- list_audio_devices
- get_audio_capabilities
- start_audio_recording
- stop_audio_recording

Screenshot:
- capture_screenshot
- save_screenshot
- export_screenshot

Media:
- get_media_metadata
- generate_thumbnail
- trim_media
- convert_media
- export_media

Storage:
- list_recordings
- delete_recording
- rename_recording
- get_storage_usage

Settings:
- get_settings
- update_settings

These are suggested interface names. Define exact request and response structures before implementing them.

### Events

Provide events for:
- recording-started
- recording-paused
- recording-resumed
- recording-stopped
- recording-progress
- audio-level
- device-changed
- capture-error
- export-progress
- export-completed
- export-failed
- storage-warning

Do not emit excessively frequent frontend events.

Throttle UI progress updates without reducing internal capture precision.

---

## 16. SETTINGS

Create comprehensive settings.

General:
- Language.
- Theme.
- Start minimized.
- Launch at startup.

Recording:
- Default resolution.
- FPS.
- Format.
- Save directory.
- Default capture source.

Video:
- Codec.
- Encoder.
- Bitrate.
- Quality preset.

Audio:
- Input device.
- Output device.
- Sample rate.
- Channels.
- Audio format.

Screenshot:
- Default format.
- Image quality.
- Save location.
- Filename template.

Performance:
- Hardware acceleration.
- Resource limits.
- Thumbnail generation.
- Background processing.

Privacy:
- Local storage.
- Optional telemetry.
- Log retention.
- No automatic cloud upload.

Shortcuts:
- Customizable global shortcuts.
- Conflict detection.
- Reset to defaults.

Persist settings across application restarts.

Validate settings before applying them.

---

## 17. SYSTEM INTEGRATION

Implement:
- System tray.
- Global keyboard shortcuts.
- Desktop notifications.
- Application startup options.
- File associations where appropriate.
- Native file dialogs.
- Open file location.
- Safe application shutdown.

The application should remain usable when minimized.

Prevent accidental application exit during active recording.

On shutdown, ask the user to stop or confirm the recording action.

---

## 18. ERROR HANDLING AND RECOVERY

Build a robust error system.

Handle:
- Capture initialization failure.
- Permission denied.
- Unsupported encoder.
- FFmpeg unavailable.
- Disk full.
- Invalid output path.
- Audio device unavailable.
- Audio device disconnected.
- Screen resolution change.
- Monitor disconnected.
- PipeWire failure.
- Portal cancellation.
- Process crash.
- Application shutdown.
- Database failure.

Use structured error types.

Provide user-friendly error messages and technical diagnostics.

### Recovery

If a recording is interrupted:
- Detect incomplete output.
- Preserve temporary files when safe.
- Attempt recoverable container finalization where possible.
- Never silently delete recoverable media.
- Show recovery status.
- Record recovery attempts.

MKV may be offered as a recovery-friendly recording container, with final MP4 conversion as an optional post-processing step.

---

## 19. SECURITY AND PRIVACY

Security is mandatory.

- Least-privilege Tauri capabilities.
- Narrowly scoped shell permissions.
- No unrestricted command execution.
- Safe path handling.
- Input validation.
- No hidden recording.
- Clear screen and microphone permission flow.
- No automatic uploads.
- No mandatory user accounts.
- No unnecessary telemetry.
- Sensitive information excluded from logs.
- Verify third-party binary integrity.
- Follow third-party software licenses.

---

## 20. TESTING REQUIREMENTS

Write tests alongside implementation.

### Rust Tests

- Unit tests.
- Service tests.
- Database tests.
- FFmpeg argument tests.
- State machine tests.
- Job lifecycle tests.
- Error handling tests.

### Frontend Tests

- Component tests.
- Store tests.
- Form validation.
- Command integration.
- Recording state synchronization.

### Integration Tests

- Video recording.
- Audio recording.
- System audio.
- Screenshot capture.
- Media conversion.
- File recovery.
- Export cancellation.

### Platform Matrix

Test:
- Windows 10.
- Windows 11.
- Ubuntu X11.
- Ubuntu Wayland.
- GNOME.
- KDE Plasma.
- Single monitor.
- Multiple monitors.
- Different resolutions.
- Different audio devices.

### Performance

Target:
- Stable long-duration recording.
- Bounded memory usage.
- Responsive interface.
- Low idle resource usage.
- Reliable timestamp synchronization.
- No unnecessary frame buffering.
- Graceful behavior under disk pressure.

Use real hardware tests for native capture. Do not mark platform functionality complete based only on mocked tests.

---

## 21. BUILD AND DISTRIBUTION

Produce:

Windows:
- Installer.
- Portable build where appropriate.

Ubuntu:
- DEB.
- AppImage.

Implement:
- Release build configuration.
- Version management.
- App icons.
- Application metadata.
- Dependency checks.
- FFmpeg packaging.
- Release documentation.

Keep Windows and Linux build instructions separate.

Document all native Linux development dependencies.

Ensure the application can be installed and launched on a clean supported system.

---

## 22. DEVELOPMENT WORKFLOW

Work incrementally.

Do not generate the entire application in one enormous code dump.

### Phase 1 — Foundation

- Initialize repository.
- Configure Tauri 2.
- Configure React and TypeScript.
- Set up UI design system.
- Implement navigation.
- Implement dashboard.
- Set up SQLite.
- Create settings architecture.

### Phase 2 — Capture Foundation

- Create platform abstraction.
- Implement Windows capture.
- Implement Linux X11 capture.
- Implement Linux Wayland portal capture.
- Create capture state machine.
- Integrate FFmpeg.
- Implement basic video recording.

### Phase 3 — Audio

- Device enumeration.
- Microphone capture.
- System audio capture.
- Audio mixer.
- Multi-track recording.
- Audio synchronization.

### Phase 4 — Screenshots

- Full-screen capture.
- Monitor capture.
- Window capture.
- Region capture.
- Screenshot annotation.
- Export.

### Phase 5 — Library and Media

- Media library.
- FFprobe.
- Thumbnail generation.
- Media preview.
- Export queue.
- Basic editor.

### Phase 6 — Advanced

- Webcam overlay.
- Cursor effects.
- Keyboard visualization.
- Audio processing.
- OCR.
- Optional transcription.

### Phase 7 — Production

- Recovery.
- Performance optimization.
- Security testing.
- Cross-platform testing.
- Installers.
- Documentation.

---

## 23. IMPORTANT CLAUDE CODE WORKING RULES

Follow these rules throughout the project.

### Rule 1: Inspect Before Coding

Inspect the repository before changing files.

If a project already exists, preserve its working architecture and identify what needs modification.

Do not blindly overwrite existing files.

### Rule 2: Architecture First

Before substantial implementation:
1. Explain the architecture.
2. Define interfaces.
3. Define data models.
4. Define module boundaries.
5. Identify platform-specific constraints.
6. Document implementation decisions.

### Rule 3: Real Functionality Only

Never create buttons that do nothing.

Never use fake recording progress.

Never simulate a successful capture.

Never use mock data in production features.

Mocks are allowed only in explicitly identified tests or development previews.

### Rule 4: Work in Small Phases

Complete one functional milestone at a time.

For each milestone:
1. Implement.
2. Compile.
3. Run tests.
4. Fix errors.
5. Verify behavior.
6. Update documentation.
7. Report completion.

Do not proceed past a blocking issue while pretending the previous milestone is complete.

### Rule 5: Cross-Platform Discipline

Keep shared business logic separate from platform-specific capture.

Do not introduce platform-specific dependencies into shared modules unnecessarily.

Test both Windows and Ubuntu builds.

### Rule 6: Avoid Unnecessary Dependencies

Prefer maintained and well-documented dependencies.

Explain why a new dependency is required.

Do not add multiple libraries for the same responsibility without a clear reason.

### Rule 7: Production-Quality Code

- Strict TypeScript.
- Typed Rust interfaces.
- No unnecessary `unwrap()` in production paths.
- Proper error handling.
- Modular services.
- Reusable components.
- Consistent naming.
- Documentation.
- Automated tests.

### Rule 8: Resource Management

Pay particular attention to:
- Memory usage.
- Frame buffers.
- Audio buffers.
- Encoder processes.
- Temporary files.
- File handles.
- Device cleanup.
- Application shutdown.

### Rule 9: Report Honestly

After each milestone, report:

- What was implemented.
- Files created.
- Files modified.
- Build status.
- Tests executed.
- Actual test results.
- Known limitations.
- Next implementation phase.

Never claim a feature works unless it has been tested appropriately.

### Rule 10: Keep Documentation Updated

Maintain:
- README.md
- ARCHITECTURE.md
- FEATURES.md
- API.md
- BUILD.md
- TROUBLESHOOTING.md
- DEVELOPMENT_PROGRESS.md

---

## 24. FIRST TASK — START THE PROJECT

Start by inspecting the current workspace.

Then perform the following:

1. Confirm installed Rust, Cargo, Node.js, pnpm and Tauri prerequisites.
2. Identify the current operating system.
3. Inspect available FFmpeg and FFprobe installations.
4. Inspect Linux display server and PipeWire availability when running on Linux.
5. Create the initial architecture documentation.
6. Initialize the Tauri 2 + React + TypeScript project.
7. Configure the folder structure.
8. Implement the premium application shell.
9. Implement navigation and dashboard.
10. Configure SQLite and settings infrastructure.
11. Verify that the project compiles and launches.

Do not attempt to implement every native recording feature during the first task.

Complete the foundation properly before moving to capture engineering.

---

## 25. DEFINITION OF DONE

The product is not complete merely because the application launches.

The application is considered production-ready only when:

- Video recording works on supported Windows systems.
- Video recording works on supported Ubuntu X11 systems.
- Video recording works on supported Ubuntu Wayland systems.
- Microphone recording works.
- System audio recording works where supported.
- Audio and video synchronization is verified.
- Screenshot capture works.
- Annotation tools work.
- Media files can be previewed.
- Export jobs work.
- Settings persist.
- Recovery is tested.
- Application packaging works.
- Installers are tested on clean systems.
- No critical unresolved defects remain.
- Documentation reflects actual implementation.

If a feature cannot be implemented reliably on a particular platform, document the limitation and provide a graceful alternative.

**FINAL INSTRUCTION:**

Build Capture Studio as a real, maintainable, high-performance, premium desktop application.

Prioritize functional recording engines and platform reliability over decorative UI or a large list of unfinished features.

Start with Phase 1 and work methodically through the development milestones. Never sacrifice engineering quality for the appearance of progress.