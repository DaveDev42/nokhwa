# Changelog

## Unreleased

### Bug Fixes

* **`ControlValueDescription::RGB::verify_setter` accepted only
  out-of-range values.** The predicate was `*v.0 >= max.0 && *v.1
  >= max.1 && *v.2 >= max.2` — i.e. it returned `true` exactly
  when *every* channel was at-or-above the upper bound, the
  inverse of what range validation should do. (`IntegerRange` and
  `FloatRange` both use `value >= min && value <= max` so RGB was
  the odd one out.) Replaced with finite-aware
  `0.0 ..= max` per channel. Existing test `verify_setter_rgb`
  documented the buggy behaviour with a `// FIXME:` comment;
  rewritten to cover the corrected semantics + NaN / infinity /
  negative rejection. `control_value_roundtrip_rgb` updated to use
  in-range values (the previous (2,3,4) over (1,1,1) only passed
  thanks to the inverted predicate).

### Cleanup

* **TODO/FIXME audit.** Removed two stale `// TODO: Update as this
  goes` / `// TODO: More` markers in `src/query.rs` (no actionable
  intent). Replaced V4L `// TODO: Respect step size` with a
  prose-comment explaining why `FrameSizeEnum::Stepwise` only
  exposes endpoints (unbounded steps would flood the UI surface);
  the proper enumeration is tracked in `TODO.md`.
* **Fixed `unused_imports` warning in `nokhwa-core/src/frame_tests.rs`.**
  The `Mjpeg` symbol is only referenced from
  `#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]` test
  blocks, but its `use` line was unconditional — every default-feature
  `cargo test -p nokhwa-core` invocation emitted
  `warning: unused import: \`Mjpeg\`` until now.

### Testing

* **Expand `tests/device_tests.rs`.** Adds four integration tests that
  exercise corners of the `nokhwa::open` / `OpenedCamera` surface
  previously left uncovered: `open_invalid_index_errors` (a far-OOB
  `CameraIndex::Index` must surface `NokhwaError`),
  `compatible_formats_nonempty` (`StreamCamera::compatible_formats()`
  must enumerate ≥1 entry on real hardware),
  `set_format_invalid_does_not_round_trip` (a `1×1@1` MJPEG request
  must either error or fail to round-trip — V4L2 may snap to the
  nearest valid format, MSMF tends to error; both behaviours pass),
  and `frame_metadata_is_stable` (consecutive `frame()`s must report a
  stable `resolution()` + `source_frame_format()` so downstream
  `Buffer::typed::<F>()` consumers don't see mid-stream renegotiation).
  Linux gets these for free on every PR via the `v4l-loopback` job;
  Windows/macOS get them on the self-hosted runners and gated PRs.

### Documentation

* **Resync example READMEs and badges with the post-0.13 API.** Three
  example READMEs (`examples/captesting`, `examples/capture`,
  `examples/threaded-capture`) still claimed to demonstrate the
  removed-in-0.13 typed-camera surface (`Camera::open::<Mjpeg>`,
  `camera.frame_typed()`, `CallbackCamera<Mjpeg>`,
  `threaded.poll_frame()`) even though the actual sources have used
  `nokhwa::open` / `OpenedCamera` / `CameraRunner` for a long time.
  Rewritten so the descriptions match the code that ships in those
  examples. The root README's git-dep snippet now pins to a tag
  (matching the `CLAUDE.md` guidance) instead of `branch = "main"`,
  and the dead `docs.rs` / `crates.io` badges on the root,
  `nokhwa-core`, and `nokhwa-tokio` READMEs have been removed — this
  fork doesn't publish to crates.io. `MIGRATING-0.13.md`'s
  forward-looking "async runner is on the 0.14.0 roadmap" line is
  updated to point at the now-shipped `nokhwa-tokio` crate.

### Performance

* **Event-driven V4L hotplug via `inotify`.** Replaces the 500 ms
  polling loop in `nokhwa-bindings-linux-v4l::hotplug` with an
  `inotify(7)` watch on `/dev/` for `IN_CREATE` / `IN_DELETE`. Worker
  thread blocks in `poll(2)` (1 s timeout for shutdown
  responsiveness) and re-`query()`s only when the kernel signals a
  device-node change — zero steady-state wake-ups, immediate
  notification on plug/unplug instead of up-to-500 ms latency.
  Mirrors the MSMF backend's `RegisterDeviceNotificationW` design
  (#173); was the last remaining 2×/sec poll thread on the Linux
  side. Public API unchanged. Added a `v4l_hotplug_take_and_steady_state`
  integration test paralleling `msmf_hotplug_take_and_steady_state`.

### Infrastructure

* **`v4l-loopback` CI: fix `videodev` + `v4l2loopback` modprobe and
  `ffmpeg` shared-lib failures on Azure 6.17 kernel.** Four
  compounding causes silently broke every PR run since the #183 era
  (masked by job-level `continue-on-error: true` — run-level green,
  job-level failure with every device-test step `skipped`):
  (1) only `linux-modules-extra-<kernel>` was installed, but on the
  Ubuntu Azure 6.17.x kernel `videodev.ko` (the V4L2 core module)
  ships in the *base* `linux-modules-<kernel>` package — the `-extra`
  split no longer carries it;
  (2) `awalsh128/cache-apt-pkgs-action` defaults to
  `execute_install_scripts: false`, so the `linux-modules*` `postinst`
  script (which runs `depmod -a`) is skipped on cache restore,
  leaving `/lib/modules/<kernel>/modules.dep` stale and causing
  `modprobe videodev` to report "not found" by name even when the
  `.ko` is on disk;
  (3) the same `execute_install_scripts: false` behaviour means that
  on a cache-hit run the `v4l2loopback-dkms` `postinst` (`dkms install`)
  never re-executes, so the compiled
  `/lib/modules/<kernel>/updates/dkms/v4l2loopback.ko` is missing for
  the running kernel and `modprobe v4l2loopback` fails with "Module
  not found in directory";
  (4) the cache action restores only listed packages, not their
  unlisted transitive shared-lib deps — `ffmpeg` was cached but its
  runtime closure (`libblas3`, `libgfortran5`, `libavfilter*` …) was
  not, so on cache hit `ffmpeg` aborted with `libblas.so.3: cannot
  open shared object file`.
  Fix: add `linux-modules-$KERNEL` (base) and `linux-headers-azure`
  to the cached apt package list; remove `ffmpeg` from the cache
  action and install it fresh via `apt-get install` in a separate
  step so apt resolves the full dependency closure (~5–10 s); insert
  an unconditional `dkms install v4l2loopback/0.12.7 -k $(uname -r)`
  step (idempotent — no-op on cold runs, ~10–15 s compile on cache
  hits) after cache restore; insert an unconditional `sudo depmod -a`
  step as belt-and-suspenders; bump the cache version key from
  `kernel-` to `kernel4-` to bust the now-incomplete cached entry.
  Job-level `continue-on-error: true` preserved.
* **`clippy::pedantic` enforced across all workspace crates; matrix lint CI.**
  Added `#![deny(clippy::pedantic)]` / `#![warn(clippy::all)]` /
  `#![allow(clippy::module_name_repetitions)]` headers to
  `nokhwa-bindings-linux-v4l` and `nokhwa-bindings-macos-avfoundation` (the
  two crates that previously had no lint policy). Fixed all 68 violations in
  `nokhwa-bindings-windows-msmf` and 20 violations in
  `nokhwa-bindings-linux-v4l`: `borrow_as_ptr` (Win32/libc FFI raw-pointer
  passing rewritten to `&raw mut`/`&raw const`), `uninlined_format_args`
  (format strings inlined), `manual_let_else`, `doc_markdown` (Win32/V4L2
  terms backtick-quoted), `get_first`, `cast_sign_loss`, `needless_ifs`,
  `explicit_iter_loop`, `iter_filter_is_some`, `iter_filter_is_ok`, and
  the renamed lint `clippy::let_underscore_drop` → `let_underscore_drop`.
  Expanded `lint.yml` from a single Ubuntu job to a three-platform matrix:
  `Clippy (linux)` (core + v4l + gstreamer + nokhwa), `Clippy (windows)`
  (msmf + nokhwa), `Clippy (macos)` (avfoundation + nokhwa). All four
  env combinations (Linux stable/nightly, Windows stable/nightly) verified
  locally. **Note:** the old `Clippy` required-status-check context in the
  branch ruleset must be updated to the new matrix names (`Clippy (linux)`,
  `Clippy (windows)`, `Clippy (macos)`) by the repo admin after merge.
* **`.gitattributes` for LF normalization.** `text=auto eol=lf` plus
  explicit rules for `*.rs` / `*.toml` / `*.md` / `*.yml` / `*.json` /
  Xcode project files (`*.pbxproj`, `*.xcscheme`, `*.plist`,
  `*.entitlements`), and binary markers for image / video / `.DS_Store`
  / `.xcuserstate`. Eliminates the cross-OS CRLF/LF noise that produced
  100+-file phantom dirty trees on WSL/Windows checkouts. `Cargo.lock`
  is also marked `linguist-generated=true` so GitHub collapses it in
  PR diffs.
* **PreToolUse Bash guards in `.claude/settings.json`** (already
  shipped via PR #178) — deterministically blocks `feat!` / `fix!` /
  `BREAKING CHANGE:` in commit/PR titles, `gh` mutation commands
  without `--repo DaveDev42/nokhwa`, and `cargo publish`. Catches
  CLAUDE.md release-policy violations at the harness level rather
  than relying on policy memory.

### Performance

* **MSMF hotplug: event-driven via `RegisterDeviceNotificationW`.**
  Replaces the 500ms-polling worker with a Win32
  `RegisterDeviceNotificationW(KSCATEGORY_VIDEO_CAMERA)`-backed
  implementation: a dedicated worker thread owns a hidden
  message-only window (`HWND_MESSAGE` parent) and pumps
  `WM_DEVICECHANGE` notifications through a static `WndProc`. On
  `DBT_DEVICEARRIVAL` / `DBT_DEVICEREMOVECOMPLETE` it re-snapshots
  via `wmf::query()` and emits `HotplugEvent::Connected` /
  `Disconnected` diffs over the same mpsc channel — identical
  `HotplugSource` trait surface as before; only the internals
  changed. `Drop` posts `WM_QUIT` to the worker thread id to break
  the `GetMessageW` loop. Zero wake-ups in the steady state (vs the
  old 2×/sec poll) and no longer any 500ms detection latency on a
  plug/unplug event.

### Removed

* **OpenCV capture backend (`input-opencv`,
  `ApiBackend::OpenCv`, `ApiBackend::Network`,
  `src/backends/capture/opencv_backend.rs`).** The GStreamer backend
  reached parity with OpenCV's video-I/O coverage across sessions 2–5:
  local capture (session 2), controls on Linux (session 3),
  `nokhwa::open` dispatch integration (session 4), and — the
  critical prerequisite — IP / RTSP / HTTP / file URL sources via
  `uridecodebin` (session 5). OpenCV's only unique value in nokhwa
  was `VideoCapture::from_file`'s URL support, now covered
  first-class.

  Removing `input-opencv` drops the `opencv` +
  `opencv/videoio` + `opencv/rgb` + `opencv/clang-runtime`
  system-dependency chain, shrinks build surface on Windows / macOS
  (no more system OpenCV install), and eliminates the ambiguous
  "which backend did I open?" situation where both the native backend
  and OpenCV would end up wrapping the same device.

  **`opencv-mat` (the separate `nokhwa-core` feature exposing
  `to_opencv_mat()` / `write_to_opencv_mat()` for CV-ecosystem
  interop) is unchanged.** Users who want to hand frames into
  `cv::Mat` for downstream image processing still enable that feature
  directly.

  **Migration.** Replace `input-opencv` with `input-gstreamer`.
  `CameraIndex::String("rtsp://...")` / `https://...` / `file://...`
  routing works identically; local-camera enumeration now goes
  through GStreamer's `DeviceMonitor` instead of OpenCV's
  backend-specific `VideoCapture(index)`. On Windows / macOS install
  GStreamer's **Complete** variant (so `mfvideosrc` / `avfvideosrc` /
  `uridecodebin` / `videoconvert` are available); on Linux install
  `libgstreamer1.0-dev` + `libgstreamer-plugins-base1.0-dev` +
  `gstreamer1.0-plugins-base` + `gstreamer1.0-libav` (for decoders)
  + `gstreamer1.0-plugins-good`.

### Features

* **GStreamer session 4 — `nokhwa::open()` dispatch integration.**
  `open(CameraIndex::String("rtsp://..."), _)` /
  `open(CameraIndex::String("http://..."), _)` /
  `open(CameraIndex::String("file://..."), _)` now short-circuit to
  the GStreamer backend regardless of which native backend is
  compiled in for the host. Native-device fast paths (V4L on Linux,
  MSMF on Windows, AVFoundation on macOS) are unchanged for
  `CameraIndex::Index(_)` and non-URL strings. GStreamer also sits
  between the native branches and OpenCV as a cross-platform
  fallback, so a build with only `input-gstreamer` enabled can still
  `open()` a local camera via `uridecodebin`'s device enumeration
  path. Hardware-verified via WSL with both `input-v4l` and
  `input-gstreamer` compiled together: `stream_camera file:///tmp/test.mp4`
  routes through GStreamer and pulls 10 frames; `stream_camera 0`
  reaches V4L native. `examples/stream_camera.rs` now accepts either
  an integer index or a URL on the CLI.

* **GStreamer session 5 — RTSP / IP / URL sources.** Passing a
  `CameraIndex::String` whose value starts with one of the known URL
  schemes (`rtsp://` / `rtsps://` / `rtmp://` / `rtmps://` / `http://`
  / `https://` / `file://` / `srt://` / `udp://` / `tcp://`) now
  dispatches the GStreamer backend through a new
  `uridecodebin uri=... ! videoconvert ! appsink` pipeline instead
  of the `DeviceMonitor` lookup path. One pipeline shape covers every
  scheme because `uridecodebin` auto-picks the right source plugin
  (`rtspsrc` / `souphttpsrc` / `filesrc` / ...) and decoder chain.
  `new()` parks the URI; `open()` builds the pipeline, waits for the
  first sample, and learns the format from the sample's caps since
  URL streams don't advertise capabilities before we connect.
  `compatible_formats()` returns an empty list until the first
  `open()`, then returns the single negotiated format. `controls()`
  / `set_control()` error in URL mode — URL streams have no V4L-style
  control surface. `set_format()` errors in URL mode — URL streams
  negotiate their own format.
  New module: `nokhwa-bindings-gstreamer::uri`. New example:
  `examples/gstreamer_url_probe.rs`. Hardware-verified via WSL
  (GStreamer 1.24.2 + `gstreamer1.0-libav` + `plugins-bad` /
  `-ugly` / `-good`):
  - Local file: `file:///tmp/test.mp4` — 5 frames at 640x480 NV12
    30fps (H.264 via `mp4mux`, decoded by `uridecodebin` through
    `avdec_h264`).
  - Remote HTTPS: `https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4`
    — 5 frames at 854x480 NV12 24fps.
  RTSP streams share the same `uridecodebin` dispatch as HTTP, so
  real RTSP camera URLs work through the same code path with no
  backend-specific logic.

  **This closes the critical prerequisite for evaluating OpenCV
  backend removal** — the IP / RTSP / file story is now covered by
  GStreamer first-class instead of going through OpenCV's
  FFmpeg-backed `VideoCapture::from_file`.

* **GStreamer session 3 — controls on Linux.** Platform-asymmetric
  camera-control support through GStreamer source elements.
  - **Linux `v4l2src`**: `controls()` enumerates the 4 `controllable`
    GObject properties (`brightness` / `contrast` / `hue` /
    `saturation`) with current values + pspec ranges. `set_control()`
    on those 4 writes immediately at any pipeline state via
    `source.set_property()`. For the rest of the V4L2 CID namespace
    (exposure / zoom / focus / pan / tilt / gain / sharpness / gamma
    / white-balance / backlight-comp) `set_control()` stages the
    value in a pending map and applies it via v4l2src's write-only
    `extra-controls` `GstStructure` on the next pipeline open
    (automatic pipeline restart if open when called).
  - **Windows `mfvideosrc` / `ksvideosrc`**: no camera-control
    properties exist on the source elements. `controls()` returns an
    empty list; `set_control()` errors for handle-less IDs. Windows
    users should use `input-msmf` for full control support.
  - **macOS `avfvideosrc`**: treated the same as Windows until
    verified on real hardware.
  - Hardware-verified on a Logitech MX Brio via WSL + `usbipd`: all
    4 live controls enumerate with their current values and
    `Brightness` 128 → 129 round-trips correctly. Known limitation:
    v4l2src's pspec reports the full `i32` range for the live-
    property ranges rather than the actual V4L2 driver range
    (MX Brio's brightness is 0–255 in V4L2). Use `input-v4l` if the
    true range matters.
  - New module `nokhwa-bindings-gstreamer::controls`;
    `examples/gstreamer_probe.rs` now lists controls + round-trips
    brightness.

> **Note on the UVC backend in this release.** Commits under the
> `feat(uvc):` prefix (sessions 1 and 2a) added a new
> `nokhwa-bindings-uvc` crate with libusb-based enumeration + UVC
> descriptor parsing. The backend was **removed before 0.14.2
> shipped** — `ApiBackend::UniversalVideoClass`, the `input-uvc`
> feature, and the bindings crate no longer exist in 0.14.2. The
> three native backends (MSMF / V4L / AVFoundation) are now
> feature-complete including hotplug, which eliminates the
> cross-platform motivation for a generic libusb-UVC path; streaming
> itself is also structurally blocked on Windows (`usbvideo.sys`
> owns the interface) and redundant with V4L / AVFoundation on
> Linux / macOS. See `TODO.md` for the full rationale. Future niche
> work (industrial / IR / UVC Processing Unit extensions) will be
> addressed by purpose-built backends rather than a generic UVC
> path. **No action required for downstream users** — anything
> relying on `input-uvc` would not have compiled in 0.14.1 anyway.

### Removed

* **UVC backend (`ApiBackend::UniversalVideoClass`, `input-uvc`,
  `nokhwa-bindings-uvc`)** before its first release. Rationale above.

### Infrastructure

* `msmf_probe` example and a long-form finding comment at the top of
  `.github/workflows/msmf-obs-virtualcam.yml` record the outcome of
  the MSMF OBS virtual-camera CI spike: OBS's virtualcam is a
  DirectShow filter, and `MFEnumDeviceSources` (the API nokhwa's MSMF
  backend uses) does not bridge DShow filters. Seeding OBS's
  first-run profile makes OBS start cleanly but does not make the
  virtual camera visible to MSMF. The workflow stays
  `workflow_dispatch`-only + `continue-on-error: true` as a
  diagnostic harness; TODO.md tracks the remaining MSMF-CI
  candidates (Windows 11 Camera Extension sample, in-harness Rust MF
  source, or a self-hosted Windows runner).

### Features

* **GStreamer session 2 — streaming on real hardware.**
  `GStreamerCaptureDevice::new()` / `open()` / `frame()` /
  `frame_raw()` / `close()` / `set_format()` are now live. The device
  owns a `source ! capsfilter ! videoconvert ! appsink` pipeline where
  `source` is whatever `Device::create_element(None)` hands us on the
  host (`v4l2src` on Linux, `mfvideosrc` on Windows, `avfvideosrc` on
  macOS), `capsfilter` pins the negotiated `video/x-raw` format,
  `videoconvert` handles I420 ↔ NV12 ↔ YUY2 transparently, and
  `AppSink` serves frames via `try_pull_sample` with
  `max_buffers=1 drop=true sync=false` for "latest frame" semantics.
  Format enumeration walks `Device::caps()` for `video/x-raw` (YUY2 /
  NV12 / GRAY8) structures and handles all three framerate shapes
  GStreamer uses: single `Fraction` (rare), `FractionList` (Linux
  `v4l2src`), and `FractionRange` (Windows `mfvideosrc` /
  `ksvideosrc` advertise e.g. `[5/1, 60/1]`). For ranges, a curated
  common-FPS list (5, 10, 15, 20, 24, 25, 30, 48, 50, 60, 90, 100,
  120) is filtered to values that fall within the advertised bounds,
  keeping `compatible_formats()` tractable instead of enumerating
  every integer fps. New module layout: `src/format.rs` (caps ↔
  `CameraFormat` mapping, 8 unit tests) and `src/pipeline.rs`
  (`PipelineHandle` lifecycle). Hardware-verified on a Logitech
  MX Brio (046d:0944) on two platforms: (a) Linux via `usbipd-win`
  forward to WSL2 Ubuntu 24.04 + GStreamer 1.24.2 (`v4l2src`) and
  (b) native Windows 11 + GStreamer 1.28.2 (`ksvideosrc`) — both
  pull 5 frames at 640x480 NV12 30fps, 460800 bytes each. New
  `examples/gstreamer_probe.rs` demonstrates end-to-end use.

* **AVFoundation hotplug (`HotplugSource` implementation).** New
  `AVFoundationHotplugContext` in `nokhwa-bindings-macos-avfoundation`,
  re-exported as
  `nokhwa::backends::hotplug::AVFoundationHotplugContext` when the
  `input-avfoundation` feature is enabled on macOS / iOS. Mirrors the
  MSMF and V4L polling impls: a dedicated background thread calls
  `device::query()` every 500ms, diffs successive snapshots keyed on
  `AVCaptureDevice.uniqueID` (which the device module already stores
  in `CameraInfo.misc`), and emits `HotplugEvent::Connected` /
  `Disconnected` through an mpsc channel wrapped in
  `Box<dyn HotplugEventPoll>`. Dropping the poll flips an
  `AtomicBool`; the thread observes it within one `POLL_INTERVAL` and
  joins. `IOKit` matching notifications would be event-driven but
  require runloop + Objective-C block plumbing for marginal benefit
  at seconds-scale latency budgets. `examples/hotplug_probe.rs` now
  picks the right backend on all three native OSes at compile time.

  This closes the \"Hotplug impls on the other backends\" follow-up —
  all three native backends (MSMF / V4L / AVFoundation) now ship a
  `HotplugSource` implementation with identical trait surface.

* **V4L hotplug (`HotplugSource` implementation).** New
  `V4LHotplugContext` in `nokhwa-bindings-linux-v4l`, re-exported as
  `nokhwa::backends::hotplug::V4LHotplugContext` when the `input-v4l`
  feature is enabled on Linux. Mirrors the MSMF polling impl: a
  dedicated background thread calls `query()` every 500ms, diffs
  successive snapshots keyed on `CameraIndex` (which maps 1:1 to
  `/dev/videoN`), and emits `HotplugEvent::Connected` / `Disconnected`
  through an mpsc channel wrapped in `Box<dyn HotplugEventPoll>`.
  Dropping the poll flips an `AtomicBool` shutdown flag; the thread
  observes it within one `POLL_INTERVAL` and joins. CI coverage lands
  via `.github/workflows/v4l-loopback.yml::V4L hotplug smoke test`
  which reloads `v4l2loopback` with `devices=2` → `devices=1` →
  `devices=2` via `modprobe -r` + `modprobe` (Ubuntu's packaged
  `v4l2loopback-ctl` lacks `add`/`delete`) and asserts the probe
  observed both event variants. `examples/hotplug_probe.rs` now picks
  the right backend at compile time (`input-msmf` on Windows,
  `input-v4l` on Linux).

* **MSMF hotplug (`HotplugSource` implementation).** New
  `MediaFoundationHotplugContext` in `nokhwa-bindings-windows-msmf`,
  re-exported as `nokhwa::backends::hotplug::MediaFoundationHotplugContext`
  when the `input-msmf` feature is enabled on Windows. Implements the
  `HotplugSource` trait introduced in 0.14: `take_hotplug_events()`
  spawns a dedicated background thread that polls `wmf::query()`
  every 500ms, diffs successive snapshots on the MSMF symbolic link
  (`CameraInfo.misc`), and emits `HotplugEvent::Connected` /
  `Disconnected` through an mpsc channel wrapped in a
  `Box<dyn HotplugEventPoll>`.

  Polling rather than `RegisterDeviceNotification` is deliberate —
  event-driven MSMF hotplug needs a hidden window plus a message pump
  on the registering thread (~200 lines of `unsafe` Win32) for a
  feature whose latency budget is seconds, not milliseconds. The poll
  loop is ten lines, never misses an event (each snapshot reflects
  the live MF device list), and joins cleanly on drop.

  Dropping the poll handle flips an `AtomicBool` shutdown flag; the
  background thread observes it within at most one `POLL_INTERVAL` and
  exits. New `examples/hotplug_probe.rs` prints events for 15 seconds
  so users can manually verify plug/unplug wiring.

* **UVC backend session 2a — format discovery on real hardware.**
  `UVCCaptureDevice::new()` now opens the libusb device and walks the
  `VideoStreaming` interface's class-specific descriptor chain
  (`VS_FORMAT_MJPEG`, `VS_FORMAT_UNCOMPRESSED` with YUY2 / NV12 GUIDs,
  `VS_FRAME_*`) to populate a cached `Vec<CameraFormat>`. New module
  `nokhwa-bindings-uvc::descriptors` parses the bytes with a small
  state machine + unit-tested fixtures. `compatible_formats()`,
  `compatible_fourcc()`, `negotiated_format()`, and `set_format()` are
  now functional; `CameraIndex::String("<bus>:<addr> <vid>:<pid>")` is
  accepted as a re-open key. Verified on a Logitech MX Brio — the
  parser surfaced 339 distinct `(resolution, format, fps)` tuples
  across MJPEG / YUYV / NV12 in under a second.

  `open()` / `frame()` / `frame_raw()` still error, but now with a
  *platform-aware* diagnostic: on Windows the message spells out that
  `usbvideo.sys` prevents `rusb::DeviceHandle::claim_interface` and
  directs users at `input-msmf`; on Linux / macOS it points at the
  session-2b streaming work still tracked in `TODO.md`. The Windows
  block is structural — the UVC backend on Windows is enumeration-only
  by design; streaming will land on Linux / macOS only.

* **New cross-platform UVC backend (session 1).** Introduces a new
  workspace crate `nokhwa-bindings-uvc` built on `rusb` / libusb, gated
  behind `input-uvc`. This release ships **device enumeration only**:
  `query(ApiBackend::UniversalVideoClass)` returns one `CameraInfo` per
  attached UVC device by walking USB config descriptors for an interface
  with class `0x0E` (Video) / subclass `0x01` (VideoControl). `CameraInfo`
  carries the `iProduct` string as `human_name`, `iManufacturer` as
  `description`, and `"<bus>:<addr> <vid>:<pid>"` in `misc` so follow-up
  streaming code can re-open the device without re-scanning.
  `UVCCaptureDevice::new()` and every `FrameSource` / `CameraDevice`
  method currently error with `NotImplementedError` /
  `UnsupportedOperationError` so the feature flag, `nokhwa_backend!`
  registration, and CI coverage can land ahead of the streaming surface.
  Streaming, format negotiation, and controls are tracked in `TODO.md`
  as sessions 2–4. Linux builds need `libusb-1.0-0-dev`; macOS and
  Windows ship the bundled libusb via rusb. Verified on a Logitech MX
  Brio (046d:0944). CI: new `Feature check (input-uvc)` job in
  `.github/workflows/test-core.yml`.

* **New cross-platform GStreamer backend (session 1).** Introduces a
  new workspace crate `nokhwa-bindings-gstreamer` built on
  `gstreamer-rs`, gated behind `input-gstreamer`. This release ships
  **device enumeration only**: `query(ApiBackend::GStreamer)` returns
  one `CameraInfo` per `Video/Source` element that the GStreamer
  `DeviceMonitor` reports with `video/x-raw` caps. `CameraInfo.human_name`
  carries the device's display name; `description` carries the element
  class (e.g. `Video/Source`); `misc` is reserved for session-2 code to
  populate with pipeline-reconstruction data.
  `GStreamerCaptureDevice::new()` and every `FrameSource` /
  `CameraDevice` method currently error with `NotImplementedError` /
  `UnsupportedOperationError(GStreamer)` so the feature flag,
  `nokhwa_backend!` registration, and CI coverage can land ahead of the
  streaming surface. Streaming, format negotiation, and controls are
  tracked in `TODO.md` as sessions 2–4. Pinned to `gstreamer = "0.23"`
  because 0.25+ requires rustc 1.92 while the workspace targets rustc
  1.89. The bindings crate also exposes an internal `backend` cargo
  feature so `cargo check -p nokhwa-bindings-gstreamer` compiles the
  stub path on machines without a system GStreamer install; the
  top-level `input-gstreamer` feature enables `backend`. CI: new
  `Feature check (input-gstreamer)` job in
  `.github/workflows/test-core.yml` installs `libgstreamer1.0-dev` +
  `gstreamer1.0-plugins-base`.

### Bug Fixes

* `OpenCvCaptureDevice::open()` now re-opens `CameraIndex::String` (IP /
  RTSP) cameras correctly. Previously it returned a hard error referencing
  the long-removed `NetworkCamera` type, so any `close()` + `open()` cycle
  on an IP camera failed even though the constructor supported it. The
  fix rebuilds `self.video_capture` via `VideoCapture::from_file(url,
  CAP_ANY)`, mirroring the constructor.

### Cleanup

* Remove a dead `#[cfg_attr(feature = "docs-features", doc(cfg(...)))]`
  annotation on `MediaFoundationCaptureDevice` in
  `nokhwa-bindings-windows-msmf`. The crate never declared `docs-features`
  (only `docs-only`) and never enabled `#![feature(doc_cfg)]`, so the
  attribute was a no-op that only produced an `unexpected_cfgs` warning.

### Infrastructure

* New `.github/workflows/msmf-obs-virtualcam.yml` — Windows-only spike
  workflow that installs OBS Studio on `windows-latest`, launches
  `obs64.exe --startvirtualcam` as a background process, and runs the
  `device-test` suite against the resulting Media Foundation source.
  Trigger is `workflow_dispatch`-only with `continue-on-error: true`
  pending first successful run; once stable the trigger will be
  promoted to `pull_request` alongside the V4L loopback job.

### Testing

* Add `control_set_get_round_trip` to `tests/device_tests.rs` (gated behind
  the `device-test` feature). Picks the first Manual-mode `IntegerRange`
  control with headroom, writes a stepped value via `set_control`, then
  re-queries `controls()` and asserts the new value round-trips. Skips
  gracefully when no writable control is available (e.g. when running
  against `v4l2loopback`), so it stays safe on the Linux CI job. Verified
  on a Windows MSMF webcam: `Brightness` 128 → 129.

### Features

* New `HotplugSource` trait and `HotplugEvent` enum (`Connected(CameraInfo)` /
  `Disconnected(CameraInfo)`) in `nokhwa-core::traits` for backend-level
  plug/unplug signals. Distinct from the per-camera `EventSource` /
  `CameraEvent::Disconnected` pair: `HotplugSource` reports devices appearing
  or disappearing for the backend as a whole, including before any camera has
  been opened.
* `HotplugSource` is intentionally **not** a `CameraDevice` supertrait — it is
  implemented by backend-wide registry/context types, not by individual
  cameras. `take_hotplug_events` mirrors the `EventSource::take_events` pattern
  and succeeds at most once per backend instance.
* `HotplugEvent` derives `PartialEq` / `Eq` / `Hash` so consumers can dedupe
  events or use them as hashmap keys. Trait-only; no backend implementations
  ship in this release. Intended consumers include Canon EDSDK
  (`EdsSetCameraAddedHandler`), Linux `inotify` on `/dev/video*`, macOS
  `IOKit` matching notifications, and Windows MSMF device-change
  notifications.

### Refactoring

* Renamed `ShutterCapture::lock` / `ShutterCapture::unlock` to `lock_ui` /
  `unlock_ui` to make intent unambiguous. These methods lock the camera's
  physical UI controls so that host-side commands have exclusive effect —
  they do not lock the shutter. The new names also align with Canon's
  EDSDK terminology (`EdsSendStatusCommand(UILock / UIUnLock)`), which
  makes the upcoming Canon DSLR backend map cleanly onto the trait.
  Downstream code that calls `ShutterCamera::lock` / `unlock` directly, or
  backends that override these `ShutterCapture` methods, must rename their
  usages. Backends that use the default no-op impls are unaffected.

## [0.14.5](https://github.com/DaveDev42/nokhwa/compare/v0.14.4...v0.14.5) (2026-04-30)


### Infrastructure

* **claude:** add PreToolUse Bash guards for release/upstream policy ([#178](https://github.com/DaveDev42/nokhwa/issues/178)) ([107f6a3](https://github.com/DaveDev42/nokhwa/commit/107f6a3d68bcf81e1355196fd0e9301bb21e23e9))
* **repo:** add .gitattributes for LF normalization ([#180](https://github.com/DaveDev42/nokhwa/issues/180)) ([b2c43fe](https://github.com/DaveDev42/nokhwa/commit/b2c43fe82f8d9abb043b72c1eaa12077ee367534))

## [0.14.4](https://github.com/DaveDev42/nokhwa/compare/v0.14.3...v0.14.4) (2026-04-22)


### Performance

* **msmf:** event-driven hotplug via RegisterDeviceNotificationW ([#173](https://github.com/DaveDev42/nokhwa/issues/173)) ([714c7dd](https://github.com/DaveDev42/nokhwa/commit/714c7dd6614415cc2926b42086619f513fdc6ed1))


### Infrastructure

* **clippy:** v4l Stream import + pedantic cleanup across workspace ([#169](https://github.com/DaveDev42/nokhwa/issues/169)) ([b9a9e52](https://github.com/DaveDev42/nokhwa/commit/b9a9e52b5675f6a7be0912c767353b9c5ec78457))
* **test-core:** cache GStreamer apt packages in check-gstreamer job ([#176](https://github.com/DaveDev42/nokhwa/issues/176)) ([667ab50](https://github.com/DaveDev42/nokhwa/commit/667ab50be5aa7d3d3249c307be70415a3025d27f))
* **v4l-loopback:** cache apt archives keyed on kernel version ([#175](https://github.com/DaveDev42/nokhwa/issues/175)) ([b248591](https://github.com/DaveDev42/nokhwa/commit/b2485914149a938d72608eacb04d2f8b3d44baf5))
* **v4l:** fix two pedantic clippy warnings ([#170](https://github.com/DaveDev42/nokhwa/issues/170)) ([c4cc193](https://github.com/DaveDev42/nokhwa/commit/c4cc1935a9799fe3107571c2b72153a5371606cc))


### Documentation

* **claude:** document that this fork never publishes to crates.io ([#172](https://github.com/DaveDev42/nokhwa/issues/172)) ([d40e3b1](https://github.com/DaveDev42/nokhwa/commit/d40e3b10414caf8c708368dee35c084e32ea56c9))
* **claude:** refresh CLAUDE.md for 0.14.x state ([#171](https://github.com/DaveDev42/nokhwa/issues/171)) ([68d1eab](https://github.com/DaveDev42/nokhwa/commit/68d1eabe275f2e3396ab955cc44a7f25212418e0))
* **todo:** close GStreamer top-level item (shipped in 0.14.3) ([#167](https://github.com/DaveDev42/nokhwa/issues/167)) ([3d7b7a2](https://github.com/DaveDev42/nokhwa/commit/3d7b7a2fa8c2c4df0d7624bcb25a8c2bd5dff147))
* **todo:** restructure to Open/Closed/Shipped ([#177](https://github.com/DaveDev42/nokhwa/issues/177)) ([d3cac32](https://github.com/DaveDev42/nokhwa/commit/d3cac320a0119f2d6c2f945745109ae4640942ef))

## [0.14.3](https://github.com/DaveDev42/nokhwa/compare/v0.14.2...v0.14.3) (2026-04-22)


### Features

* **gstreamer:** session 3 — controls via gst_properties + extra-controls ([#162](https://github.com/DaveDev42/nokhwa/issues/162)) ([a744c56](https://github.com/DaveDev42/nokhwa/commit/a744c56623b6b216b830c6236da2c959e1af0637))
* **gstreamer:** session 4 — nokhwa::open() dispatch integration ([#164](https://github.com/DaveDev42/nokhwa/issues/164)) ([06444b3](https://github.com/DaveDev42/nokhwa/commit/06444b388ff7b68ea5062dcb44d6c493399199fe))
* **gstreamer:** session 5 — RTSP / IP / URL sources via uridecodebin ([#163](https://github.com/DaveDev42/nokhwa/issues/163)) ([6432e3f](https://github.com/DaveDev42/nokhwa/commit/6432e3f2bc8f0fa756adc8ef12f7877716f56ea0))


### Refactoring

* remove OpenCV capture backend ([#166](https://github.com/DaveDev42/nokhwa/issues/166)) ([3ed2f65](https://github.com/DaveDev42/nokhwa/commit/3ed2f6540d346e737305e83af562dcbf28eca218))

## [0.14.2](https://github.com/DaveDev42/nokhwa/compare/v0.14.1...v0.14.2) (2026-04-21)


### Features

* add HotplugSource trait for backend-level plug/unplug events ([#140](https://github.com/DaveDev42/nokhwa/issues/140)) ([682f750](https://github.com/DaveDev42/nokhwa/commit/682f75082786c28681ebec58787ddea2c4b97ee0))
* **avf:** implement HotplugSource via polling device::query() ([#158](https://github.com/DaveDev42/nokhwa/issues/158)) ([a4d96df](https://github.com/DaveDev42/nokhwa/commit/a4d96df01bb29bd3fb19bab44e02c6f4ed48dd45))
* **gstreamer:** session 1 — nokhwa-bindings-gstreamer crate + query() enumeration ([#150](https://github.com/DaveDev42/nokhwa/issues/150)) ([46f309b](https://github.com/DaveDev42/nokhwa/commit/46f309be71758bf57ffd758f4565fffc9e080846))
* **gstreamer:** session 2 — streaming via appsink on real hardware ([#160](https://github.com/DaveDev42/nokhwa/issues/160)) ([4629cab](https://github.com/DaveDev42/nokhwa/commit/4629cab5d413ca462bcd842b34757d8ed6b62bfa))
* **msmf:** implement HotplugSource via polling wmf::query() ([#153](https://github.com/DaveDev42/nokhwa/issues/153)) ([4e19a61](https://github.com/DaveDev42/nokhwa/commit/4e19a6124d78608b5e34e8b2ac7fb212ee40eb9a))
* **uvc:** session 1 — nokhwa-bindings-uvc crate + query() enumeration ([#149](https://github.com/DaveDev42/nokhwa/issues/149)) ([e01e825](https://github.com/DaveDev42/nokhwa/commit/e01e825f7823a40b1dea587ac12df917238ec3cf))
* **uvc:** session 2a — descriptor parsing + format discovery on real hardware ([#152](https://github.com/DaveDev42/nokhwa/issues/152)) ([fbc4a86](https://github.com/DaveDev42/nokhwa/commit/fbc4a8607d4427294d99f366e2be1129fb48af05))
* **v4l:** implement HotplugSource via polling enum_devices() ([#156](https://github.com/DaveDev42/nokhwa/issues/156)) ([f8700b3](https://github.com/DaveDev42/nokhwa/commit/f8700b3b52b93097385cf0f3b8571cfb3ea492ec))


### Bug Fixes

* **gstreamer:** enumerate FractionRange framerates (Windows mfvideosrc/ksvideosrc) ([#161](https://github.com/DaveDev42/nokhwa/issues/161)) ([eed176c](https://github.com/DaveDev42/nokhwa/commit/eed176cec2f609475be48e7576f634c620543d0a))
* **opencv:** re-open CameraIndex::String (IP camera) via from_file ([#147](https://github.com/DaveDev42/nokhwa/issues/147)) ([9fedf4d](https://github.com/DaveDev42/nokhwa/commit/9fedf4d5b736619a18328e6fdca953f509f6efb2))


### Refactoring

* remove UVC backend before 0.14.2 ships ([#159](https://github.com/DaveDev42/nokhwa/issues/159)) ([65a61ef](https://github.com/DaveDev42/nokhwa/commit/65a61ef063713c4715f6ffb3838bf90d5938ef81))
* rename ShutterCapture lock/unlock to lock_ui/unlock_ui ([#141](https://github.com/DaveDev42/nokhwa/issues/141)) ([190a387](https://github.com/DaveDev42/nokhwa/commit/190a387fa7fe2ec93d07faeff731e8259729d7a4))


### Infrastructure

* **msmf:** drop dead docs-features cfg_attr in capture.rs ([#143](https://github.com/DaveDev42/nokhwa/issues/143)) ([a9d9387](https://github.com/DaveDev42/nokhwa/commit/a9d9387c247dc6c85f7fb130e5a89fbefeb32f83))
* spike MSMF device tests on windows-latest via OBS virtual camera ([#148](https://github.com/DaveDev42/nokhwa/issues/148)) ([6f0937c](https://github.com/DaveDev42/nokhwa/commit/6f0937ce94fcb9b377f3ea57cc2fcf883b554dc9))
* **v4l:** remove silent-skip paths from hotplug smoke ([#157](https://github.com/DaveDev42/nokhwa/issues/157)) ([fe5d0a2](https://github.com/DaveDev42/nokhwa/commit/fe5d0a2787cb1fe25141d9f05a4278b4c4c085d6))


### Documentation

* **gstreamer:** record session-2 prerequisites (usbipd + WSL setup) ([#155](https://github.com/DaveDev42/nokhwa/issues/155)) ([1b7f786](https://github.com/DaveDev42/nokhwa/commit/1b7f7863040bdc88a3d06dc38e5a0b734f7b25d7))
* **msmf-obs:** abandon session 2, document DShow vs MF structural blocker ([#154](https://github.com/DaveDev42/nokhwa/issues/154)) ([e46f404](https://github.com/DaveDev42/nokhwa/commit/e46f40455f4cb83e73fd8ac5a8ce52a69816f6a8))
* **todo:** annotate MSMF spike session 2 with first-run failure mode ([#151](https://github.com/DaveDev42/nokhwa/issues/151)) ([0f5203a](https://github.com/DaveDev42/nokhwa/commit/0f5203a9812e798a500a4d0e8dbeae8d39021284))
* **todo:** rescope backlog — close Network/IP, flag OpenCV re-open bug ([#146](https://github.com/DaveDev42/nokhwa/issues/146)) ([bee66fa](https://github.com/DaveDev42/nokhwa/commit/bee66fae81c70de5adfa649b387659f17f089b26))
* **todo:** scope virtual-camera CI — close macOS, keep Windows spike ([#145](https://github.com/DaveDev42/nokhwa/issues/145)) ([09d865a](https://github.com/DaveDev42/nokhwa/commit/09d865a90bcf3505e0e880ba748d6749dc4ddc4b))


### Testing

* **device:** add control set/get round-trip on real hardware ([#144](https://github.com/DaveDev42/nokhwa/issues/144)) ([de39d9e](https://github.com/DaveDev42/nokhwa/commit/de39d9eae0f052fcdebf3daea078e8babfec8fb8))

## [0.14.1](https://github.com/DaveDev42/nokhwa/compare/v0.14.0...v0.14.1) (2026-04-15)

### Performance

* `OpenCvCaptureDevice::raw_frame_vec` now reuses a single per-device
  frame buffer and performs the BGR→RGB swizzle via `chunks_exact_mut`
  instead of allocating a fresh `Vec<u8>` and pushing byte-by-byte on
  every frame. The return value switches from `Cow::Owned` to
  `Cow::Borrowed`; the public `Cow<'_, [u8]>` signature is unchanged.

### Infrastructure

* Hardware-gated V4L tests now run on every pull request via a new
  `v4l-loopback` CI job. The job loads `v4l2loopback` as `/dev/video0`,
  pumps an `ffmpeg` YUYV test pattern into it, and runs
  `cargo test --features input-v4l,device-test,runner` against the
  synthetic device. Closes the V4L `nokhwa::open` dispatch regression
  gap (previously a self-hosted camera was required). See
  `.github/workflows/v4l-loopback.yml`. The job is marked
  `continue-on-error` so DKMS / kernel-ABI drift on GitHub-hosted
  runners cannot block merges.

## [0.14.0](https://github.com/DaveDev42/nokhwa/compare/v0.13.3...v0.14.0) (2026-04-15)

### ⚠ BREAKING CHANGES (API)

* **`CameraSession` removed.** The unit-struct namespace is gone; open a
  camera via the free function `nokhwa::open(index, req)` instead of
  `CameraSession::open(index, req)`. `OpenRequest`, `OpenedCamera`, and
  the per-capability wrappers are unchanged. See
  [MIGRATING-0.14.md#opening-a-camera](MIGRATING-0.14.md#opening-a-camera).

### ⚠ BREAKING CHANGES (behavior)

* **`RunnerConfig` now defaults to bounded channels.** Capacities are
  `frames_capacity = 4`, `pictures_capacity = 8`, `events_capacity = 32`,
  with `Overflow::DropNewest` as the default policy. In 0.13 the
  channels were unbounded, so a slow consumer would queue forever; in
  0.14 the slowest-moving item is silently dropped according to the
  policy. **Mitigation:** set any of the three capacity fields to `0` to
  restore the 0.13 unbounded behavior. See
  [MIGRATING-0.14.md#bounded-runner-channels](MIGRATING-0.14.md#bounded-runner-channels).

### Features

* `CameraRunner` channels are bounded by default. `RunnerConfig`
  re-introduces `frames_capacity`, `pictures_capacity`, `events_capacity`
  and a new `Overflow` policy enum (`DropNewest` / `DropOldest`). Setting
  a capacity to `0` keeps the 0.13 unbounded semantics.
* New `take_frames` / `take_pictures` / `take_events` accessors on
  `CameraRunner` hand ownership of a receiver to the caller while the
  worker thread keeps running — enabling wrappers such as
  `nokhwa-tokio::TokioCameraRunner`.
* New workspace crate **`nokhwa-tokio`** provides `TokioCameraRunner`,
  an async wrapper around `CameraRunner` exposing
  `tokio::sync::mpsc::Receiver`s and async-safe `Drop` (dropping inside a
  tokio runtime does not block the caller).
* **Restored `input-opencv` backend.** Migrated `OpenCvCaptureDevice` to the
  0.13.0 `CameraDevice` + `FrameSource` trait split, re-registered via
  `nokhwa_backend!`, and removed the `compile_error!` gate. `nokhwa::open`
  now falls through to the opencv branch when no native `input-*` backend
  matches the current target/feature configuration. OpenCV still requires
  the system library at build time (`opencv/clang-runtime`); CI coverage is
  tracked as a separate follow-up.
* **New `examples/live_view` demo.** Standalone example crate that opens a
  camera via `nokhwa::open`, spawns `CameraRunner` on a worker thread, and
  paints decoded RGB frames to a `minifb` window. Replaces the ggez-based
  live-view demo lost in the 0.13.0 refactor — `minifb` is a lighter fit
  for the "pump frames into a window" use case.

### Infrastructure

* New integration test `tests/session.rs::hybrid_camera_with_events_delivers_poller`
  exercises the `EventSource` arm of the `nokhwa_backend!` macro from an
  external-crate-style newtype, validating that the extension point is
  usable by third-party backends such as a Canon EDSDK binding.
* `tests/device_tests.rs` (gated `device-test`) ported to the post-0.13
  `nokhwa::open` / `OpenedCamera` API. Covers `query`, stream capture,
  control enumeration, and `CameraRunner` smoke testing on real hardware.
* Cross-platform `cargo doc --features docs-only,docs-nolink` now builds
  on macOS and Linux: `nokhwa-bindings-windows-msmf` exposes a non-Windows
  `MediaFoundationCaptureDevice` stub mirroring the existing
  `V4LCaptureDevice` stub.
* CI: new `Feature check (input-opencv)` job on Ubuntu runs
  `cargo check --features input-opencv` with `libopencv-dev` + `libclang-dev`
  installed, so regressions in the restored OpenCV backend are caught on
  every PR.

### Documentation

* New `MIGRATING-0.14.md` covering the `CameraSession` → `open()` change.
* New "Using nokhwa from async runtimes" section in the crate docs.
* `nokhwa-tokio/examples/tokio_runner.rs` demonstrates pulling frames
  with `.recv().await`.

## [0.13.3](https://github.com/DaveDev42/nokhwa/compare/v0.13.2...v0.13.3) (2026-04-14)


### Infrastructure

* adopt patch-only release policy and force 0.13.3 bump ([#129](https://github.com/DaveDev42/nokhwa/issues/129)) ([8ea5156](https://github.com/DaveDev42/nokhwa/commit/8ea51563754ec67918b6267713f19474b75ea2f0))

## [0.13.2](https://github.com/DaveDev42/nokhwa/compare/v0.13.1...v0.13.2) (2026-04-14)


### ⚠ BREAKING CHANGES

* replace CameraSession with free nokhwa::open (0.14.0 group C) ([#124](https://github.com/DaveDev42/nokhwa/issues/124))

### Features

* **opencv:** migrate input-opencv backend to 0.13.0 trait split (0.14.0 group B) ([#126](https://github.com/DaveDev42/nokhwa/issues/126)) ([c1caa7d](https://github.com/DaveDev42/nokhwa/commit/c1caa7d0cd739dc6f1a182ede34ec0b4d5a68638))
* replace CameraSession with free nokhwa::open (0.14.0 group C) ([#124](https://github.com/DaveDev42/nokhwa/issues/124)) ([ea8b65a](https://github.com/DaveDev42/nokhwa/commit/ea8b65a24a0e58a4a961afa2e89f7485e2e86fab))


### Infrastructure

* **ci:** force release-please to 0.13.2 via release-as override ([#128](https://github.com/DaveDev42/nokhwa/issues/128)) ([7b4d72a](https://github.com/DaveDev42/nokhwa/commit/7b4d72a9720bf80534a021a07c20aa6703cfc226))

## [0.13.1](https://github.com/DaveDev42/nokhwa/compare/v0.13.0...v0.13.1) (2026-04-13)


### Features

* **runner:** bounded channels + nokhwa-tokio crate (0.14.0 group A) ([#123](https://github.com/DaveDev42/nokhwa/issues/123)) ([af21d3c](https://github.com/DaveDev42/nokhwa/commit/af21d3c7e8d6323dbadae48f72c62509fe82e134))


### Infrastructure

* **ci:** remove release-as override after v0.13.0 ([#121](https://github.com/DaveDev42/nokhwa/issues/121)) ([a5d7aa4](https://github.com/DaveDev42/nokhwa/commit/a5d7aa4a521ca07644635392c9c8e551603ce266))

## [0.13.0](https://github.com/DaveDev42/nokhwa/compare/v0.12.0...v0.13.0) (2026-04-13)

### ⚠ BREAKING CHANGES

* Replaced `CaptureBackendTrait` with four capability-based traits:
  `CameraDevice`, `FrameSource`, `ShutterCapture`, `EventSource`.
* Removed `Camera<F>` and `CallbackCamera<F>`. Replaced by `CameraSession`
  (returning an `OpenedCamera` enum with `Stream`, `Shutter`, `Hybrid`
  variants) and `CameraRunner` (threaded helper).
* Renamed feature `output-threaded` → `runner`; dropped `parking_lot`
  and `arc-swap` deps that only backed the removed `CallbackCamera`.
* `input-opencv` backend temporarily disabled pending migration to the
  new traits (enabling it now triggers a `compile_error!`).
* Backend trait methods `refresh_camera_format`, `resolution`,
  `frame_rate`, `frame_format`, `set_resolution`, `set_frame_rate`,
  `set_frame_format`, and single-control `camera_control(id)` are
  removed. Use `negotiated_format()` / `set_format(CameraFormat)` and
  `controls()` instead.
* See `MIGRATING-0.13.md` for a full step-by-step guide.

### Features

* New `CameraEvent` type and `EventPoll` trait for camera events
  (disconnect, capture error, will-shut-down).
* `CameraRunner` channel-based threaded helper behind the `runner`
  feature, with per-variant loops (stream / shutter / hybrid) and
  configurable queue sizes.
* `nokhwa_backend!` macro for custom-backend crates to declare
  their capability set and obtain the internal `AnyDevice` impl.
* New `testing` feature on `nokhwa-core` providing `MockFrameSource`,
  `MockShutter`, and `MockHybrid` backends for integration tests.
* `CameraSession::open` now dispatches to `V4LCaptureDevice` on Linux
  alongside the AVFoundation and Media Foundation branches ([#119](https://github.com/DaveDev42/nokhwa/issues/119)) ([a5eea5c](https://github.com/DaveDev42/nokhwa/commit/a5eea5c61cbcd4cb1c81c68f00e6c37bcefb872a)).
  The `V4LCaptureDevice<'a>` lifetime parameter was removed; the
  `MmapStream` handle is stored as `'static`. See the struct-level
  docs on `V4LCaptureDevice` for the soundness argument.

### Bug Fixes

* **post-0.13.0:** MSMF MTA, runner shutdown, doc fixes ([#118](https://github.com/DaveDev42/nokhwa/issues/118)) ([a243d51](https://github.com/DaveDev42/nokhwa/commit/a243d5106fbb0473a7ae75015fa591c50d65a04a))

### Infrastructure

* `wgpu` helpers (`RawTextureData`, `raw_texture_layout`) moved from
  `nokhwa_core::traits` to a dedicated `nokhwa_core::wgpu` module.
* Workspace version bumped to 0.13.0.
* Pre-commit hook gained a `NOKHWA_SKIP_CLIPPY` escape hatch used
  during the trait-split transition; the workspace clippy is clean at
  release time.
* **ci:** force release-please to 0.13.0 via release-as override ([#120](https://github.com/DaveDev42/nokhwa/issues/120)) ([1d8b85a](https://github.com/DaveDev42/nokhwa/commit/1d8b85a804aa6b69d9e3f749a7f3e3a24ca84435))
* **ci:** remove release-as override and last-release-sha after v0.12.0 ([#115](https://github.com/DaveDev42/nokhwa/issues/115)) ([bcaef97](https://github.com/DaveDev42/nokhwa/commit/bcaef9727880fa5a6d42c171f6614c8ec1bc6455))

### Documentation

* Added `MIGRATING-0.13.md` covering the 0.12 → 0.13 migration.
* Rewrote top-level `lib.rs` doc comments and README quick-start.
* Migrated all examples (`capture`, `captesting`, `setting`,
  `threaded-capture`) to the new API and added minimal
  `examples/stream_camera.rs` and `examples/runner.rs` at the
  workspace root.

### Additional breaking changes

* `RunnerConfig` has been trimmed to three fields (`poll_interval` —
  renamed from `tick`, `event_tick`, and the new `shutter_timeout`
  defaulting to 5 s, replacing the previously hard-coded 200 ms).
  The vestigial `frames_capacity` / `pictures_capacity` /
  `events_capacity` / `overflow` fields and the `Overflow` enum were
  removed because `std::sync::mpsc::channel` is unbounded. Bounded
  channels with an overflow policy are tracked for 0.14.
* `CameraSession` is now a unit struct; the no-op `CameraSession::new`
  constructor was removed. `CameraSession::open(index, req)` is
  unchanged.

### Diagnostics

* `HybridCamera::from_device` / `CameraRunner::spawn_hybrid` and the
  `CameraRunner` worker thread joins now log event-poller init
  failures and worker panics via `log::warn!` (gated on the `logging`
  feature) instead of swallowing or burying them in an `Option`.

### Internal

* Hidden macro-internal items (`from_device`, `AnyDevice`,
  `HybridBackend`, `CAP_*`) are now `#[doc(hidden)]` throughout.
* Deleted the unused `__nokhwa_cap_bit!` helper macro.
* Replaced stale `tests/device_tests.rs` body with a placeholder
  pending migration to the 0.13 API.

## [0.12.0](https://github.com/DaveDev42/nokhwa/compare/v0.11.0...v0.12.0) (2026-04-12)


### ⚠ BREAKING CHANGES

* type-safe decode API (0.12.0) ([#85](https://github.com/DaveDev42/nokhwa/issues/85))
* restructure error types with structured context, fix UninitializedError typo ([#47](https://github.com/DaveDev42/nokhwa/issues/47))
* remove deprecated API methods (new_with, set_camera_format) ([#44](https://github.com/DaveDev42/nokhwa/issues/44))

### Features

* add frame_texture_raw() for native-format GPU textures ([#50](https://github.com/DaveDev42/nokhwa/issues/50)) ([a83bcd5](https://github.com/DaveDev42/nokhwa/commit/a83bcd5d380d9bb868adf64de88c0600b4c9d50a))
* add frame_timeout() method for bounded frame capture ([#49](https://github.com/DaveDev42/nokhwa/issues/49)) ([daf3475](https://github.com/DaveDev42/nokhwa/commit/daf3475c533941c0d2f6b00b9d39fe5917cda5c3))
* add structured logging behind optional feature flag ([#76](https://github.com/DaveDev42/nokhwa/issues/76)) ([485eebc](https://github.com/DaveDev42/nokhwa/commit/485eebcc2397cceb6f2d2c95e6b9ddaecef85d8b))
* add TimestampKind to Buffer for platform-aware timestamp semantics ([#48](https://github.com/DaveDev42/nokhwa/issues/48)) ([a89b7d8](https://github.com/DaveDev42/nokhwa/commit/a89b7d8e2fbf872d7b20ee171ca0e7da138ada80))
* **core:** port OpenCV Mat conversion to Frame&lt;F&gt; API ([#94](https://github.com/DaveDev42/nokhwa/issues/94)) ([8a5dab9](https://github.com/DaveDev42/nokhwa/commit/8a5dab917fbfb94d236b153a85237ecb80ef26e0))
* type-safe decode API (0.12.0) ([#85](https://github.com/DaveDev42/nokhwa/issues/85)) ([6874fb6](https://github.com/DaveDev42/nokhwa/commit/6874fb6b20cdd282a487977f345653df88a87408))


### Bug Fixes

* address code review — filter logic, stream state, thread lifecycle, API typo ([#39](https://github.com/DaveDev42/nokhwa/issues/39)) ([4f3098e](https://github.com/DaveDev42/nokhwa/commit/4f3098ec5294627def6bddeb879d86e15be208dc))
* **ci:** prevent release-please from bumping to 1.0.0 on breaking changes ([#91](https://github.com/DaveDev42/nokhwa/issues/91)) ([38b879d](https://github.com/DaveDev42/nokhwa/commit/38b879dfbc17a2690bbe952a98d3461e5cac9731))
* **ci:** switch release-please to simple type for workspace compatibility ([#63](https://github.com/DaveDev42/nokhwa/issues/63)) ([4bd8243](https://github.com/DaveDev42/nokhwa/commit/4bd8243eda4ae623c743822ad20f91fdd1dab11a))
* replace unsafe impl Send for Camera with type-level Send bound ([#45](https://github.com/DaveDev42/nokhwa/issues/45)) ([200b6b3](https://github.com/DaveDev42/nokhwa/commit/200b6b355af0ef67b36b7a6885a5ad50432470ae))
* revert workspace version to 0.11.0, will release 0.12.0 when ready ([#87](https://github.com/DaveDev42/nokhwa/issues/87)) ([0cdf3f4](https://github.com/DaveDev42/nokhwa/commit/0cdf3f4119fae9d526f80a74ab6e6042497dc4ec))
* update release-please last-release-sha to current main, cleanup TODO ([#102](https://github.com/DaveDev42/nokhwa/issues/102)) ([252ccf9](https://github.com/DaveDev42/nokhwa/commit/252ccf905244bfd64451404cbffa15d87f078347))


### Performance

* add SIMD-optimized pixel format conversion for YUYV and BGR ([#58](https://github.com/DaveDev42/nokhwa/issues/58)) ([81c4670](https://github.com/DaveDev42/nokhwa/commit/81c4670e525f76f1cf6d3cdd9f3bb71c6c4be3f9))
* add zero-copy Buffer constructors, eliminate redundant frame copies ([#56](https://github.com/DaveDev42/nokhwa/issues/56)) ([d74887b](https://github.com/DaveDev42/nokhwa/commit/d74887be6c549f1d697f903c24f688d2615c6571))
* eliminate double copy in AVFoundation frame capture pipeline ([#52](https://github.com/DaveDev42/nokhwa/issues/52)) ([9ae0609](https://github.com/DaveDev42/nokhwa/commit/9ae0609063cdbd36e227d9af0d4b0803efd0b6a5))
* inline YUV-to-RGB conversion in NV12 decoder ([#67](https://github.com/DaveDev42/nokhwa/issues/67)) ([576e19b](https://github.com/DaveDev42/nokhwa/commit/576e19b09baeabf0fa149240b14fbc8302c74f3a))
* optimize NV12 decoder with pre-computed UV offsets ([#53](https://github.com/DaveDev42/nokhwa/issues/53)) ([6f3929e](https://github.com/DaveDev42/nokhwa/commit/6f3929e1c253f57426cb9724828289605a98f201))
* reduce CallbackCamera lock contention with lock-free last_frame ([#59](https://github.com/DaveDev42/nokhwa/issues/59)) ([b84add4](https://github.com/DaveDev42/nokhwa/commit/b84add42a85d6d9e8351cea4f337d5e3180b48d7))
* SIMD optimizations for all pixel format converters ([#98](https://github.com/DaveDev42/nokhwa/issues/98)) ([17ac2bb](https://github.com/DaveDev42/nokhwa/commit/17ac2bbc4293c3f19c3e3f3abd25a2af9b67949a))
* use unchecked indexing in NV12 scalar decoder hot loops ([#70](https://github.com/DaveDev42/nokhwa/issues/70)) ([031dfe2](https://github.com/DaveDev42/nokhwa/commit/031dfe2b095f9ad1bd0e0e2e5e7f2eb5fd213f73))
* use unchecked indexing in YUYV scalar decoder hot loops ([#73](https://github.com/DaveDev42/nokhwa/issues/73)) ([73576aa](https://github.com/DaveDev42/nokhwa/commit/73576aab676b4aa88a5d231ae869821b9fe12270))
* use unchecked indexing in YUYV/NV12 scalar decoder hot loops ([#69](https://github.com/DaveDev42/nokhwa/issues/69)) ([52cfa3f](https://github.com/DaveDev42/nokhwa/commit/52cfa3feb406bc1beca34bd6b6108021b1eb075d))


### Refactoring

* **core:** split simd.rs into domain-based module directory ([#101](https://github.com/DaveDev42/nokhwa/issues/101)) ([37bbb7a](https://github.com/DaveDev42/nokhwa/commit/37bbb7a501e37fd3392d06effea8ceec615b99e0))
* extract common backend logic to nokhwa-core, normalize query function names ([#80](https://github.com/DaveDev42/nokhwa/issues/80)) ([1adddeb](https://github.com/DaveDev42/nokhwa/commit/1adddeb767e83966ec03d8038910d0aa8069bbfa))
* improve Camera API ergonomics ([#77](https://github.com/DaveDev42/nokhwa/issues/77)) ([2b9a4d7](https://github.com/DaveDev42/nokhwa/commit/2b9a4d706e7992598f4a2c769bd63719945804b2))
* **macos:** reduce unsafe surface area with safe wrapper methods ([#78](https://github.com/DaveDev42/nokhwa/issues/78)) ([5b834f7](https://github.com/DaveDev42/nokhwa/commit/5b834f79634787382eb6cc4adf0d19e0eda048e2))
* replace backend dispatch macros with explicit factory functions ([#43](https://github.com/DaveDev42/nokhwa/issues/43)) ([f9fe9f6](https://github.com/DaveDev42/nokhwa/commit/f9fe9f69e8d28879f440a9b48c077834c0a180b8))
* restructure error types with structured context, fix UninitializedError typo ([#47](https://github.com/DaveDev42/nokhwa/issues/47)) ([3fd2fe0](https://github.com/DaveDev42/nokhwa/commit/3fd2fe0811aad9e5aed7f9d0d25c466c82791c3e))
* restructure OpenDeviceError with named fields ([#66](https://github.com/DaveDev42/nokhwa/issues/66)) ([21c1471](https://github.com/DaveDev42/nokhwa/commit/21c1471d9b808e559b397688d8229107836ac8a1))
* simplify recently changed code — reduce duplication across Frame/Camera/SIMD ([#105](https://github.com/DaveDev42/nokhwa/issues/105)) ([935ffc2](https://github.com/DaveDev42/nokhwa/commit/935ffc248976d20c869f3d0005c83ced14dc6cc8))


### Infrastructure

* add Claude Code local files and planning artifacts to .gitignore ([#89](https://github.com/DaveDev42/nokhwa/issues/89)) ([6c7f19e](https://github.com/DaveDev42/nokhwa/commit/6c7f19e89fd5ec1578b870a04aacf9aff6e182c0))
* **ci:** correct release-please baseline to actual v0.11.0 commit ([#111](https://github.com/DaveDev42/nokhwa/issues/111)) ([66ae79c](https://github.com/DaveDev42/nokhwa/commit/66ae79c51904a1517d898c7fd2f63bc108b6a18f))
* **ci:** force release-please to 0.12.0 via release-as override ([#113](https://github.com/DaveDev42/nokhwa/issues/113)) ([4bddb78](https://github.com/DaveDev42/nokhwa/commit/4bddb78e6c982847b7a05b6949031d9225d5a906))
* **ci:** set release-please baseline to v0.11.0 commit ([#109](https://github.com/DaveDev42/nokhwa/issues/109)) ([f5c9a4f](https://github.com/DaveDev42/nokhwa/commit/f5c9a4f5e81fc70c5aaee34eab33004a325897a2))
* **examples:** migrate to 0.12.0 Frame&lt;F&gt; / Camera&lt;F&gt; API ([#106](https://github.com/DaveDev42/nokhwa/issues/106)) ([d9bb67f](https://github.com/DaveDev42/nokhwa/commit/d9bb67f2c97037cb5ff522507ee0400b5e01136f))
* gitignore Claude Code runtime files ([#82](https://github.com/DaveDev42/nokhwa/issues/82)) ([7108742](https://github.com/DaveDev42/nokhwa/commit/7108742987709041431b7e27aea3c8b2ee0e8391))
* improve feature flag discoverability with compile-time checks and documentation ([#74](https://github.com/DaveDev42/nokhwa/issues/74)) ([5af7c79](https://github.com/DaveDev42/nokhwa/commit/5af7c798dcb0a3af671a01d3aa6aa3b81ba1e29c))
* **main:** release 0.11.1 ([#64](https://github.com/DaveDev42/nokhwa/issues/64)) ([c6b0053](https://github.com/DaveDev42/nokhwa/commit/c6b0053ac6c74620280fc417c671069502bcb4b5))
* make image crate dependency optional, gated behind decoding feature ([#81](https://github.com/DaveDev42/nokhwa/issues/81)) ([addbe44](https://github.com/DaveDev42/nokhwa/commit/addbe4495f4c7f0fa3cae8a3143a2c3b24bc4a3c))
* remove deprecated API methods (new_with, set_camera_format) ([#44](https://github.com/DaveDev42/nokhwa/issues/44)) ([5c4a7cc](https://github.com/DaveDev42/nokhwa/commit/5c4a7cc1fa39aefb96554e6c425a2b5534622578))
* replace once_cell with std::sync::LazyLock ([#75](https://github.com/DaveDev42/nokhwa/issues/75)) ([f436290](https://github.com/DaveDev42/nokhwa/commit/f43629095b582950531fb154290a6ad69f992da3))
* set up release-please for automated patch versioning ([#62](https://github.com/DaveDev42/nokhwa/issues/62)) ([3f606f1](https://github.com/DaveDev42/nokhwa/commit/3f606f1abcd674ecfefafaefe595fb1ab83a3d38))


### Documentation

* add 0.13.0 roadmap — separate streaming vs still-image capture ([#99](https://github.com/DaveDev42/nokhwa/issues/99)) ([5f2cd7c](https://github.com/DaveDev42/nokhwa/commit/5f2cd7c944a174ed05ce8c824c0d3c57f2aea3f0))
* add comprehensive SIMD performance items to TODO.md ([#95](https://github.com/DaveDev42/nokhwa/issues/95)) ([4633cf2](https://github.com/DaveDev42/nokhwa/commit/4633cf269e869e92894aca6fe22fc66edfa07d7a))
* add new improvement items to TODO.md, add gw TODO rule to CLAUDE.md ([#72](https://github.com/DaveDev42/nokhwa/issues/72)) ([e5c1a7f](https://github.com/DaveDev42/nokhwa/commit/e5c1a7f65f3a16047be931600aa8eae26d5222c8))
* add performance improvement items to TODO.md ([#51](https://github.com/DaveDev42/nokhwa/issues/51)) ([1516922](https://github.com/DaveDev42/nokhwa/commit/15169220b97882aa140cabe2aa0ec6458bc2a6f8))
* add simd.rs module split task, mark SIMD items completed ([#100](https://github.com/DaveDev42/nokhwa/issues/100)) ([bd66460](https://github.com/DaveDev42/nokhwa/commit/bd66460b63933020c302c0f996342f3531ce2ae7))
* add simplify review, docs update, examples update, benchmarks to TODO.md ([#104](https://github.com/DaveDev42/nokhwa/issues/104)) ([10a5509](https://github.com/DaveDev42/nokhwa/commit/10a55090fca3a601c144911283b239bb05080e49))
* clean up TODO.md — remove all completed items from recent PRs ([#83](https://github.com/DaveDev42/nokhwa/issues/83)) ([11427f3](https://github.com/DaveDev42/nokhwa/commit/11427f32545a3cfa8332b37d49de9509e9938dd6))
* **core:** replace ignore doc-tests with compilable examples ([#40](https://github.com/DaveDev42/nokhwa/issues/40)) ([187b182](https://github.com/DaveDev42/nokhwa/commit/187b182a102df4d3138ddf64d6784c515ae6a892))
* fix stale YUYV comments, update TODO.md for completed performance items ([#60](https://github.com/DaveDev42/nokhwa/issues/60)) ([e6033e6](https://github.com/DaveDev42/nokhwa/commit/e6033e6e53b17084b6c5c5354099608d6f6f02f4))
* improve Camera, lib.rs, RequestedFormat, CaptureBackendTrait, and examples documentation ([#79](https://github.com/DaveDev42/nokhwa/issues/79)) ([6f5bf2d](https://github.com/DaveDev42/nokhwa/commit/6f5bf2d6249e1369575435ff9c4dda291cd05a2a))
* mark CallbackCamera Drop panic as already fixed in TODO.md ([#42](https://github.com/DaveDev42/nokhwa/issues/42)) ([c773b12](https://github.com/DaveDev42/nokhwa/commit/c773b12b0ff95e051c47d908c2f9294c4d27d983))
* remove completed items from TODO.md ([#96](https://github.com/DaveDev42/nokhwa/issues/96)) ([5aae15f](https://github.com/DaveDev42/nokhwa/commit/5aae15f307004fe1483d3ed3a75d591114e3c4a6))
* remove completed items from TODO.md for readability ([#61](https://github.com/DaveDev42/nokhwa/issues/61)) ([afdfb10](https://github.com/DaveDev42/nokhwa/commit/afdfb103beb07b981fa9ee6246d191756657aaa1))
* remove completed OpenDeviceError and NV12 inline items from TODO.md ([#68](https://github.com/DaveDev42/nokhwa/issues/68)) ([fe8e1d4](https://github.com/DaveDev42/nokhwa/commit/fe8e1d4ee33d681dda3a4f00af1e335a09d40a66))
* separate MJPEG unit tests from integration tests in TODO.md ([#92](https://github.com/DaveDev42/nokhwa/issues/92)) ([fd7052d](https://github.com/DaveDev42/nokhwa/commit/fd7052d3f6b35c5c1452cb28d85b84dcbf5278d2))
* update CHANGELOG and TODO for NV12 decoder optimization ([#53](https://github.com/DaveDev42/nokhwa/issues/53)) ([#54](https://github.com/DaveDev42/nokhwa/issues/54)) ([ee88024](https://github.com/DaveDev42/nokhwa/commit/ee88024f2f950846bbf571c3e70c6642adf9ee97))
* update README, lib.rs, and add migration guide for 0.12.0 API ([#108](https://github.com/DaveDev42/nokhwa/issues/108)) ([3ebfe03](https://github.com/DaveDev42/nokhwa/commit/3ebfe03c6b7d16b2968b16d2c911de842d312263))
* update TODO.md — reflect NV12 unchecked indexing done in [#70](https://github.com/DaveDev42/nokhwa/issues/70) ([#71](https://github.com/DaveDev42/nokhwa/issues/71)) ([f3c5a2e](https://github.com/DaveDev42/nokhwa/commit/f3c5a2e32f066d9f4d80d692011e6f8d91ce4bc0))
* update TODO.md — remove completed items, clean up stale entries ([#65](https://github.com/DaveDev42/nokhwa/issues/65)) ([3c6f5bb](https://github.com/DaveDev42/nokhwa/commit/3c6f5bb485c0491d6d44148754e76135c0c4fb2f))
* update TODO.md with structural improvement items from project review ([#41](https://github.com/DaveDev42/nokhwa/issues/41)) ([c7333eb](https://github.com/DaveDev42/nokhwa/commit/c7333eb9f542b6786606ee5d3dacae962548ef43))


### Testing

* add format conversion, control round-trip, and robustness tests ([#46](https://github.com/DaveDev42/nokhwa/issues/46)) ([1afaa2b](https://github.com/DaveDev42/nokhwa/commit/1afaa2b7bcbbdd9ecd169d313bfb90d20780676a))
* **core:** add MJPEG positive correctness and robustness unit tests ([#93](https://github.com/DaveDev42/nokhwa/issues/93)) ([8e87327](https://github.com/DaveDev42/nokhwa/commit/8e87327cb311a2dd1eeca3524dbf1a21eaea94b6))

## 0.12.0 (unreleased)

### Breaking Changes

* **Type-safe decode API**: `Camera` and `CallbackCamera` are now generic over `CaptureFormat` (`Camera<F: CaptureFormat = Mjpeg>`). New `Camera::open::<F>()` constructor selects format at compile time.
* **Removed `FormatDecoder` trait and `pixel_format.rs`**: The old `Buffer::decode_image::<RgbFormat>()` pattern is replaced by `Frame<F>` with `IntoRgb`, `IntoRgba`, and `IntoLuma` conversion traits.
* **Removed `decoding` feature flag**: The `image` crate is now always required. MJPEG decoding is controlled by the `mjpeg` feature (enabled by default).
* **`ApiBackend::Custom(String)` variant added**: `ApiBackend` is no longer `Copy` (now `Clone` only). `Camera::backend()` returns `&ApiBackend`.
* **`Buffer` API reduced**: Removed `decode_image()`, `decode_image_to_buffer()`, `decode_opencv_mat()`, `decode_into_opencv_mat()` methods. Use `Frame<F>` conversions instead.
* **`RequestedFormat::new::<F>()`** now takes a `CaptureFormat` type instead of `FormatDecoder`.

### Features

* **`CaptureFormat` trait + 6 marker ZSTs**: `Yuyv`, `Nv12`, `Mjpeg`, `Gray`, `RawRgb`, `RawBgr` in `nokhwa_core::format_types`.
* **`Frame<F>` typed frame handle**: Lazy conversion via `into_rgb()`, `into_rgba()`, `into_luma()` returning `RgbConversion`, `RgbaConversion`, `LumaConversion` structs. `materialize()` performs the actual pixel conversion.
* **Compile-time format safety**: `Frame<Gray>` does not implement `IntoRgb` or `IntoRgba` — attempting grayscale-to-RGB conversion is a compile error.
* **Direct Y-channel extraction**: `buf_yuyv_extract_luma()` and `buf_nv12_extract_luma()` extract luminance without intermediate RGB conversion.
* **`Camera::frame_typed()`**: Returns `Frame<F>` for type-checked frame capture.

### Refactoring

* **SIMD module split**: `nokhwa-core/src/simd.rs` (2,150+ lines) split into `simd/` module directory organized by conversion domain (`bgr_to_rgb`, `yuyv_to_rgb`, `nv12_to_rgb`, `rgb_to_rgba`, `yuyv_extract_luma`, `rgb_to_luma`). Pure refactor — no behavior changes.

### Bug Fixes

* **wgpu**: Fixed `frame_texture()` writing to `mip_level: 1` instead of `mip_level: 0` (base level).

### Docs

* **README**: Rewrote Quick Start for the `Camera::open::<F>()` / `frame_typed()` / `into_rgb().materialize()` flow and added a compile-fail demo for `Frame<Gray>`.
* **nokhwa-core**: Added module-level overview covering `Buffer`, `Frame<F>`, `CaptureFormat` markers, and the `IntoRgb`/`IntoRgba`/`IntoLuma` lazy-conversion traits.
* **nokhwa**: Expanded top-level module docs with Key Types for `Camera<F>`, `CallbackCamera<F>`, and `Frame<F>`.
* **MIGRATION-0.12.md**: New migration guide from 0.11.x with API map, before/after examples, removed items, and a format picker table.

### Cleanup

* **examples**: Updated `examples/` to the 0.12.0 typed API (`Camera::open::<F>`, `Camera<F>::frame_typed`, `Frame<F>` + `IntoRgb`/`IntoRgba`); added READMEs for `captesting`, `decoder_test`, and `threaded-capture`; each example now opts out of the root workspace so it builds standalone.

### Infrastructure

* **Benchmarks**: Added criterion benchmarks for pixel format conversions (BGR→RGB, YUYV→RGB/RGBA, NV12→RGB/RGBA, RGB→Luma, YUYV Y-extraction) at 640×480, 1920×1080, and 3840×2160. Compares SIMD vs scalar with per-benchmark correctness checks. Gated behind the internal `bench` Cargo feature in `nokhwa-core` (not part of the stable API).

## [0.11.1](https://github.com/DaveDev42/nokhwa/compare/v0.11.0...v0.11.1) (2026-04-11)


### Features

* add structured logging behind optional feature flag ([#76](https://github.com/DaveDev42/nokhwa/issues/76)) ([485eebc](https://github.com/DaveDev42/nokhwa/commit/485eebcc2397cceb6f2d2c95e6b9ddaecef85d8b))


### Bug Fixes

* **ci:** switch release-please to simple type for workspace compatibility ([#63](https://github.com/DaveDev42/nokhwa/issues/63)) ([4bd8243](https://github.com/DaveDev42/nokhwa/commit/4bd8243eda4ae623c743822ad20f91fdd1dab11a))


### Performance

* inline YUV-to-RGB conversion in NV12 decoder ([#67](https://github.com/DaveDev42/nokhwa/issues/67)) ([576e19b](https://github.com/DaveDev42/nokhwa/commit/576e19b09baeabf0fa149240b14fbc8302c74f3a))
* use unchecked indexing in NV12 scalar decoder hot loops ([#70](https://github.com/DaveDev42/nokhwa/issues/70)) ([031dfe2](https://github.com/DaveDev42/nokhwa/commit/031dfe2b095f9ad1bd0e0e2e5e7f2eb5fd213f73))
* use unchecked indexing in YUYV scalar decoder hot loops ([#73](https://github.com/DaveDev42/nokhwa/issues/73)) ([73576aa](https://github.com/DaveDev42/nokhwa/commit/73576aab676b4aa88a5d231ae869821b9fe12270))
* use unchecked indexing in YUYV/NV12 scalar decoder hot loops ([#69](https://github.com/DaveDev42/nokhwa/issues/69)) ([52cfa3f](https://github.com/DaveDev42/nokhwa/commit/52cfa3feb406bc1beca34bd6b6108021b1eb075d))


### Refactoring

* extract common backend logic to nokhwa-core, normalize query function names ([#80](https://github.com/DaveDev42/nokhwa/issues/80)) ([1adddeb](https://github.com/DaveDev42/nokhwa/commit/1adddeb767e83966ec03d8038910d0aa8069bbfa))
* improve Camera API ergonomics ([#77](https://github.com/DaveDev42/nokhwa/issues/77)) ([2b9a4d7](https://github.com/DaveDev42/nokhwa/commit/2b9a4d706e7992598f4a2c769bd63719945804b2))
* **macos:** reduce unsafe surface area with safe wrapper methods ([#78](https://github.com/DaveDev42/nokhwa/issues/78)) ([5b834f7](https://github.com/DaveDev42/nokhwa/commit/5b834f79634787382eb6cc4adf0d19e0eda048e2))
* restructure OpenDeviceError with named fields ([#66](https://github.com/DaveDev42/nokhwa/issues/66)) ([21c1471](https://github.com/DaveDev42/nokhwa/commit/21c1471d9b808e559b397688d8229107836ac8a1))


### Infrastructure

* gitignore Claude Code runtime files ([#82](https://github.com/DaveDev42/nokhwa/issues/82)) ([7108742](https://github.com/DaveDev42/nokhwa/commit/7108742987709041431b7e27aea3c8b2ee0e8391))
* improve feature flag discoverability with compile-time checks and documentation ([#74](https://github.com/DaveDev42/nokhwa/issues/74)) ([5af7c79](https://github.com/DaveDev42/nokhwa/commit/5af7c798dcb0a3af671a01d3aa6aa3b81ba1e29c))
* make image crate dependency optional, gated behind decoding feature ([#81](https://github.com/DaveDev42/nokhwa/issues/81)) ([addbe44](https://github.com/DaveDev42/nokhwa/commit/addbe4495f4c7f0fa3cae8a3143a2c3b24bc4a3c))
* replace once_cell with std::sync::LazyLock ([#75](https://github.com/DaveDev42/nokhwa/issues/75)) ([f436290](https://github.com/DaveDev42/nokhwa/commit/f43629095b582950531fb154290a6ad69f992da3))
* set up release-please for automated patch versioning ([#62](https://github.com/DaveDev42/nokhwa/issues/62)) ([3f606f1](https://github.com/DaveDev42/nokhwa/commit/3f606f1abcd674ecfefafaefe595fb1ab83a3d38))


### Documentation

* add new improvement items to TODO.md, add gw TODO rule to CLAUDE.md ([#72](https://github.com/DaveDev42/nokhwa/issues/72)) ([e5c1a7f](https://github.com/DaveDev42/nokhwa/commit/e5c1a7f65f3a16047be931600aa8eae26d5222c8))
* clean up TODO.md — remove all completed items from recent PRs ([#83](https://github.com/DaveDev42/nokhwa/issues/83)) ([11427f3](https://github.com/DaveDev42/nokhwa/commit/11427f32545a3cfa8332b37d49de9509e9938dd6))
* improve Camera, lib.rs, RequestedFormat, CaptureBackendTrait, and examples documentation ([#79](https://github.com/DaveDev42/nokhwa/issues/79)) ([6f5bf2d](https://github.com/DaveDev42/nokhwa/commit/6f5bf2d6249e1369575435ff9c4dda291cd05a2a))
* remove completed OpenDeviceError and NV12 inline items from TODO.md ([#68](https://github.com/DaveDev42/nokhwa/issues/68)) ([fe8e1d4](https://github.com/DaveDev42/nokhwa/commit/fe8e1d4ee33d681dda3a4f00af1e335a09d40a66))
* update TODO.md — reflect NV12 unchecked indexing done in [#70](https://github.com/DaveDev42/nokhwa/issues/70) ([#71](https://github.com/DaveDev42/nokhwa/issues/71)) ([f3c5a2e](https://github.com/DaveDev42/nokhwa/commit/f3c5a2e32f066d9f4d80d692011e6f8d91ce4bc0))
* update TODO.md — remove completed items, clean up stale entries ([#65](https://github.com/DaveDev42/nokhwa/issues/65)) ([3c6f5bb](https://github.com/DaveDev42/nokhwa/commit/3c6f5bb485c0491d6d44148754e76135c0c4fb2f))

## 0.11.0 (unreleased, fork: DaveDev42/nokhwa)

## Breaking
- Removed deprecated `Camera::new_with()` (use `Camera::new()` or `Camera::with_backend()` instead)
- Removed deprecated `Camera::set_camera_format()` and `CallbackCamera::set_camera_format()` (use `set_camera_request()` instead)
- Renamed bindings crates: `nokhwa-bindings-macos` → `nokhwa-bindings-macos-avfoundation`, `nokhwa-bindings-windows` → `nokhwa-bindings-windows-msmf`, `nokhwa-bindings-linux` → `nokhwa-bindings-linux-v4l`
- Unified all workspace crate versions to `0.11.0` (workspace version inheritance)
- Moved `CaptureBackendTrait` impl from root crate wrappers into bindings crates (consistent with Linux pattern)
- Replaced `flume` with `std::sync::mpsc` (API compatible but different error types)
- Removed `camera-sync-impl` feature flag — `Camera` is now `Send` at the type level via `Box<dyn CaptureBackendTrait + Send>`. The `output-threaded` feature no longer pulls in `camera-sync-impl`. `Camera::with_custom()` now requires `Box<dyn CaptureBackendTrait + Send>` (callers passing a non-Send backend will get a compile error).
- `NokhwaError` variant changes: `UnitializedError` renamed to `UninitializedError`; `GeneralError(String)`, `OpenStreamError(String)`, `ReadFrameError(String)`, `StreamShutdownError(String)` changed from tuple to struct variants with structured context fields

## Performance
- Optimized NV12 decoder: pre-computed UV row offset and output row offset outside inner loop, consolidated UV indexing to eliminate redundant per-pixel division
- Inlined BT.601 YUV-to-RGB conversion in NV12 scalar decoder, eliminating per-pixel helper function calls and intermediate array allocations
- CallbackCamera threading overhaul: eliminated simultaneous multi-lock, fixed memory ordering (SeqCst → Release/Acquire), added thread join in Drop
- CallbackCamera: replaced `Mutex<Buffer>` last_frame with lock-free `ArcSwap`, reducing per-frame lock acquisitions from 3 to 1
- Replaced `to_vec()` + sort allocations with zero-allocation `max_by_key` iterators in `RequestedFormat::fulfill()`
- Deduplicated Windows Media Foundation format enumeration (~80 lines removed)
- Removed unnecessary `Vec::default()` allocations in CallbackCamera

## Refactoring
- **Extracted common backend logic into nokhwa-core**: added `FrameFormat::from_fourcc()`/`to_fourcc()` for canonical FourCC string mapping, `KnownCameraControl::as_index()`/`from_index()`/`from_platform_id()`/`to_platform_id()` for shared control-ID mapping via lookup tables. V4L2 backend updated to use shared helpers. Normalized query function names: `query_media_foundation_descriptors()` → `query()` in MSMF, `query_avfoundation()` → `query()` in AVFoundation.
- Renamed `camera_controls_string()` → `camera_controls_by_name()` and `camera_controls_known_camera_controls()` → `camera_controls_by_id()` on `Camera` and `CallbackCamera` (old names kept as `#[deprecated]` aliases)
- Fixed 'fufill' → 'fulfill' typo in `set_camera_request()` error message
- **Restructured error types**: replaced `String`-based variants (`GeneralError`, `OpenStreamError`, `ReadFrameError`, `StreamShutdownError`) with structured fields (`backend: Option<ApiBackend>`, `format: Option<FrameFormat>`). Binding crates now populate context. Added helper constructors for backwards compatibility.
- Fixed `UnitializedError` typo → `UninitializedError`
- **macOS: migrated from `objc`/`cocoa-foundation` to `objc2`/`block2`** — eliminated all 186 deprecation warnings, reduced dependencies from 6 to 3
- Split macOS bindings monolith (2,422 lines) into 6 focused modules (ffi, util, types, callback, device, session)
- Fixed UB: `from_raw_parts_mut` → `from_raw_parts` in CVPixelBuffer callback

## Features
- Added convenience constructors `Camera::new_with_highest_resolution()` and `Camera::new_with_highest_framerate()`
- Added optional structured logging behind `logging` feature flag — replaces `dbg!()`/`eprintln!()` with `log` crate (`log::warn!`, `log::error!`)
- Added sensor capture timestamp support across all backends (cherry-picked from upstream l1npengtul/nokhwa#234)
  - `Buffer::with_timestamp()` constructor and `Buffer::capture_timestamp()` accessor
  - macOS: `CMSampleBufferGetPresentationTimeStamp` → wall clock conversion
  - Linux: `v4l2_buffer.timestamp` → wall clock conversion
  - Windows: `IMFSample::GetSampleTime` → wall clock conversion
- Added `TimestampKind` enum for platform-aware timestamp semantics
  - Variants: `Capture`, `Presentation`, `MonotonicClock`, `WallClock`, `Unknown`
  - `Buffer::with_timestamp()` now accepts `Option<(Duration, TimestampKind)>`
  - New `Buffer::capture_timestamp_with_kind()` accessor; `capture_timestamp()` remains backward-compatible
  - Each backend tags its timestamps: macOS → `Presentation`, Linux → `WallClock`, Windows → `MonotonicClock`
  - `#[non_exhaustive]` for future extensibility; serde support behind `serialize` feature

## Bug Fixes
- Fixed NV12 pixel formats (420 biplanar YCbCr) incorrectly mapped to `FrameFormat::YUYV` instead of `FrameFormat::NV12` in macOS bindings
- Fixed `lockForConfiguration:` error pointer passed by value (NSError** must be pointer-to-pointer) — ObjC runtime could never write back errors
- Fixed NV12 output format requesting 10-bit variant instead of 8-bit in `AVCaptureVideoDataOutput::set_frame_format`
- Fixed `AVCaptureVideoCallback` leaking ObjC delegate and GCD dispatch queue (added `Drop` impl)
- Fixed `wanted_decoder` filter inconsistently applied in `HighestResolution`/`HighestFrameRate` format selection
- Fixed several macOS AVFoundation bugs discovered during objc2 migration:
  - `maxWhiteBalanceGain` read as wrong type (UB)
  - `BacklightComp` setter sending wrong selector
  - `Gain` setter extracting wrong value type
  - `TorchMode` inverted flag logic
  - `data_len()` sending unregistered selector (runtime crash)
  - `CGFloat` incorrectly defined as f32 on 64-bit (should be f64)
- Poisoned mutex errors now logged instead of silently swallowed in CallbackCamera

## Infrastructure
- Added cross-platform CI: lint, build-matrix, test-core, device-test workflows
- Added pre-commit hook (cargo fmt + clippy)
- Added 24 unit tests for nokhwa-core
- Clippy pedantic: 30 errors → 0
- Made `image` crate dependency optional in both `nokhwa` and `nokhwa-core`, gated behind the `decoding` feature flag. Building without `decoding` no longer pulls in the `image` crate, reducing compile times and dependency count for users who only need raw frame capture.

## Cleanup
- Replaced `flume` crate with `std::sync::mpsc` to reduce external dependencies (all channel usages migrated in library and examples)
- Replaced `core-media-sys` / `core-video-sys` crate dependencies with direct FFI declarations in `ffi.rs`, eliminating legacy `objc 0.2` and `metal 0.18` transitive dependencies
- Removed unused dependencies from nokhwa-core: `usb_enumeration`, `regex`, `cocoa-foundation`, `core-foundation`, `once_cell`
- Replaced `once_cell::sync::Lazy` with `std::sync::LazyLock` in Windows bindings, removing `once_cell` dependency from `nokhwa-bindings-windows-msmf`
- Removed unused `once_cell` dependency from `nokhwactl` example
- Removed dead code: empty `VirtualBackendTrait`, commented-out module declarations, obsolete code blocks
- Removed obsolete `make-npm.sh` (JS bindings removed in 0.10.0)

## 0.10.0
- Split core types and traits into `nokhwa-core`
  - Now you can use `nokhwa`'s Camera types in your own packages, to e.g. create `nokhwa` extensions or use `nokhwa`'s decoders.  
- Removed support for JS Bindings
  - This is due to lack of support for non-C style enums in `wasm-bindgen`. 
  - You can still use `nokhwa` in the browser, you just can't use it from JS.
- New CameraControl API
  - Deprecated `raw_camera_control` API
- New RequestedFormat API
- Removed Network Camera 
  - Network Camera is now supported through OpenCV Camera instead.
- New Buffer API
- New PixelFormat API
- Callback Camera: Removed `Result` from the `index()` and `camera_info()` API.
- AVFoundation Improvements
- Split V4L2 into its own crate
- New Formats:
  - NV12
  - RAWRGB
  - GRAY
- Added warning about decoding on main thread reducing performance
- After a year in development, We hope it was worth the wait.

## 0.9.0
- Fixed Camera Controls for V4L2
- Disabled UVC Backend.
- Added polling and last frame to `ThreadedCamera`
- Updated the `CameraControl` related Camera APIs

## 0.8.0
- Media Foundation Access Violation fix (#13)

## 0.7.0
- Bumped some dependencies.

## 0.5.0
 - Fixed `msmf`
 - Relicensed to Apache-2.0

## 0.4.0
- Added AVFoundation, MSMF, WASM
- `.get_info()` returns a `&CameraInfo`
- Added Threaded Camera
- Added JSCamera
- Changed `new` to use `CaptureAPIBackend::Auto` by default. Old functionally still possible with `with_backend()`
- Added `query()`, which uses `CaptureAPIBackend::Auto` by default.
- Fixed/Added examples

## 0.3.2
- Bumped `ouroboros` to avoid potential UB
- [INTERNAL] Removed `Box<T>` from many internal struct fields of `UVCCaptureDevice`

## 0.3.1
- Added feature hacks to prevent gstreamer/opencv docs.rs build failure

## 0.3.0
- Added `query_devices()` to query available devices on system
- Added `GStreamer` and `OpenCV` backends
- Added `NetworkCamera`
- Added WGPU Texture and raw buffer write support
- Added `capture` example
- Removed `get_` from all APIs. 
- General documentation fixes
- General bugfixes/performance enhancements


## 0.2.0
First release
- UVC/V4L backends
- `Camera` struct for simplification
- `CaptureBackendTrait` to simplify writing backends
