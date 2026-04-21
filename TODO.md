# TODO

## High Priority
(None)

## Medium Priority
(None)

## Low Priority
- [ ] Expand platform integration tests (requires physical camera, gated behind `device-test` feature). Port in `tests/device_tests.rs` covers query, multi-frame streaming, control enumeration, and `CameraRunner` smoke.
  - Linux is now auto-covered on every PR via `v4l2loopback` (see `.github/workflows/v4l-loopback.yml`) — the V4L `nokhwa::open` dispatch regression is CI-guarded.
  - [x] Camera control round-trip on real hardware (set → get value verification). `tests/device_tests.rs::control_set_get_round_trip` picks the first Manual-mode `IntegerRange` control with headroom, writes a stepped value, re-reads, and asserts. Verified on a Windows MSMF webcam (2026-04-20) against `Brightness` 128 → 129. Skips gracefully on devices that only expose Automatic/ReadOnly controls (e.g. `v4l2loopback`).
  - [ ] **MSMF** CI coverage on a GitHub-hosted `windows-latest` runner (spike). Drafted in `.github/workflows/msmf-obs-virtualcam.yml` (trigger: `workflow_dispatch`-only, `continue-on-error: true`). MSMF manually verified on a Windows dev box 2026-04-20 (4/4 device tests pass).
    - [x] **Session 1** — workflow drafted (#148): `choco install obs-studio` + `Start-Process obs64.exe --startvirtualcam --minimize-to-tray --disable-shutdown-check` + `Get-PnpDevice` diagnostics + device-tests + failure-log dump.
    - [x] ~~**Session 2** — seed OBS profile to skip first-run blocker~~ — **abandoned.** Local reproduction on 2026-04-21 proved the first-run hypothesis wrong *and* surfaced a deeper structural blocker. With a fully seeded `%APPDATA%\obs-studio\basic\profiles\Untitled\` + `basic\scenes\Untitled.json` + `global.ini (FirstRun=false, Profile=Untitled, SceneCollection=Untitled)`, OBS on a stock `winget install OBSProject.OBSStudio` install *does* complete startup (`==== Startup complete ====` observed in the log) and *does* run `--startvirtualcam` — but the OBS virtual camera is a **DirectShow filter** registered by `obs-virtualcam-module64.dll` (CLSID `{A3FCE0F5-3493-419F-958A-ABA1250EC20B}` under `HKLM\SOFTWARE\Classes\CLSID`), and **does not surface via `MFEnumDeviceSources`**. Confirmed with a local `msmf_probe` example: with OBS running + virtual camera active, `query(ApiBackend::MediaFoundation)` still returns exactly one device — the physical MX Brio. `MediaFoundation` and `DirectShow` are disjoint enumeration namespaces on modern Windows; OBS's DShow filter is invisible to the MF path nokhwa takes. No amount of OBS configuration changes that.
    - [ ] **Session 3** — find an MF-native virtual camera for GH-hosted CI. Candidates:
      - **Windows 11 Camera Extension sample** (smourier/VCamSample) — registers as a native MF source. Blocker: requires a code-signing certificate that GH Actions cannot provide without secrets.
      - **Ship a minimal Rust MF source** in the test harness — feasible but nontrivial (~500 LOC of `windows` crate FFI).
      - **Self-hosted Windows runner** with a real USB webcam — the same path `macos-camera` takes for AVFoundation. Most pragmatic if the team has the hardware.
      - **Accept the gap.** The `workflow_dispatch`-only + `continue-on-error: true` existing workflow stays as a diagnostic harness (comment header updated to reflect the DShow/MF finding) but MSMF CI coverage remains the documented gap it was before the spike started.
  - [x] ~~macOS GH-hosted virtual camera~~ — **not feasible**. Modern virtual cameras require system extensions that must be codesigned + notarized + installed from `/Applications`, which GitHub-hosted macOS runners cannot supply (no Apple Developer credentials). AVFoundation CI coverage remains the responsibility of the self-hosted `macos-camera` runner.

## Performance
(None)

## Follow-ups on shipped features
- [ ] Event-driven MSMF hotplug. Current impl polls `wmf::query()`
  every 500ms, which is simple and reliable but wakes up 2× per
  second. If the extra thread wake-ups become a concern, port to
  `RegisterDeviceNotification(KSCATEGORY_VIDEO_CAMERA)` with a
  hidden window + message pump; the hotplug API surface doesn't
  change. Tracked as a perf optimisation, not a correctness gap.
- [x] ~~Hotplug impls on the other backends.~~ All three native
  backends now ship `HotplugSource`:
  - [x] **V4L** polling impl (2026-04-21): `V4LHotplugContext` in
    `nokhwa-bindings-linux-v4l::hotplug` mirrors MSMF — 500ms poll of
    `v4l::context::enum_devices()` keyed on `CameraIndex`. CI coverage
    lands via `.github/workflows/v4l-loopback.yml::V4L hotplug smoke
    test` which reloads `v4l2loopback` with `devices=2` → `devices=1`
    → `devices=2` via `modprobe -r` + `modprobe` (Ubuntu's packaged
    `v4l2loopback-ctl` lacks `add`/`delete`) and asserts the probe
    observed `Connected(` + `Disconnected(` events. `inotify`-based
    event-driven impl is a follow-up if the 2×/sec wake becomes a
    concern — same perf-only gap the MSMF impl has.
  - [x] **AVFoundation** polling impl (2026-04-21):
    `AVFoundationHotplugContext` in
    `nokhwa-bindings-macos-avfoundation::hotplug` mirrors MSMF/V4L —
    500ms poll of `device::query()` keyed on
    `AVCaptureDevice.uniqueID` (stored in `CameraInfo.misc`).
    Compilation verified on macOS via the `Build (macos)` CI job.
    End-to-end hardware verification awaits the self-hosted
    `macos-camera` runner or a manual `hotplug_probe` run. `IOKit`
    matching-notification event-driven impl is a follow-up if the
    2×/sec wake becomes a concern — same perf-only gap as MSMF/V4L.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously ~839 lines). Multi-session rollout in progress:
  - [x] **Session 1** (2026-04-20): `nokhwa-bindings-gstreamer` workspace crate pinned to `gstreamer = "0.23"` (0.25+ requires rustc 1.92 which exceeds our 1.89 toolchain). `query(ApiBackend::GStreamer)` uses `DeviceMonitor` filtered to `Video/Source` with `video/x-raw` caps; each `Device` becomes a `CameraInfo` with `display_name` + `device_class`. `GStreamerCaptureDevice::new()` and every `FrameSource` method currently error — streaming lands in session 2. CI: `.github/workflows/test-core.yml::check-gstreamer` installs `libgstreamer1.0-dev` + `gstreamer1.0-plugins-base`. The bindings crate has an optional `backend` cargo feature so `cargo check` without GStreamer installed still compiles the stub path; the top-level `input-gstreamer` feature flips `backend` on.
  - [x] **Session 2 prerequisites** (2026-04-21): Windows dev-box prep for the streaming session. Installed `usbipd-win 5.3.0` via `winget install dorssel.usbipd-win` (requires UAC). WSL2 Ubuntu 24.04 already available. Inside WSL installed `libgstreamer1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good pkg-config build-essential` (GStreamer 1.24.2) plus `rustup default stable` (rustc 1.95.0). Verified `cargo check -p nokhwa-bindings-gstreamer --features backend` links against the system GStreamer and `cargo test -p nokhwa-bindings-gstreamer --features backend` passes the smoke test in WSL. MX Brio bus ID on this box is `7-4`; `usbipd list` recognises it. Remaining steps for a future session: `usbipd bind --busid 7-4` (one-time, admin) then `usbipd attach --wsl --busid 7-4` (no admin needed once bound) to forward the webcam to WSL for end-to-end streaming tests, and `usbipd detach --busid 7-4` to return it to Windows.
  - [ ] **Session 2 — Streaming.** Build a pipeline `v4l2src / mfvideosrc / avfvideosrc ! video/x-raw ! appsink`, pull frames via `gstreamer_app::AppSink::pull_sample`, and feed them into `Buffer`. Format enumeration via `Device::caps()` filter on `video/x-raw`.
  - [ ] **Session 3**: Controls via `gst-properties` on the source element (best-effort; GStreamer does not expose a canonical `KnownCameraControl` → property map the way UVC/MSMF/V4L2 do).
  - [ ] **Session 4** (optional): `nokhwa::open()` dispatch integration.
- [ ] Re-implement UVC backend (cross-platform via libusb, previously ~561 lines). Multi-session rollout in progress:
  - [x] **Session 1** (2026-04-20): `nokhwa-bindings-uvc` workspace crate using `rusb`; `query(ApiBackend::UniversalVideoClass)` enumerates UVC devices. `UVCCaptureDevice::new()` and every `FrameSource` method errored with `NotImplementedError`.
  - [x] **Session 2a** (2026-04-20, this release): `UVCCaptureDevice::new()` now opens the libusb device, walks the `VideoStreaming` interface's `VS_FORMAT_MJPEG` / `VS_FORMAT_UNCOMPRESSED` (YUY2 / NV12) and `VS_FRAME_*` descriptors, and fills a cached `Vec<CameraFormat>`. `compatible_formats()`, `compatible_fourcc()`, `negotiated_format()`, and `set_format()` are now live. `CameraIndex::String("<bus>:<addr> <vid>:<pid>")` is accepted as a re-open key (matches the value `query()` puts in `CameraInfo.misc`). `open()` / `frame()` / `frame_raw()` still error with a platform-aware diagnostic via `streaming_unsupported()`. Verified on a Logitech MX Brio (046d:0944): 339 distinct `(resolution, format, fps)` tuples across MJPEG / YUYV / NV12 discovered in <1s. See `nokhwa-bindings-uvc/src/descriptors.rs` for the parser + unit tests.
  - [ ] **Session 2b — Streaming (Linux/macOS).** Implement isochronous reads: `claim_interface` on the `VideoStreaming` interface, detach the kernel driver if present, `UVC_SET_CUR(VS_PROBE_CONTROL)` + `UVC_SET_CUR(VS_COMMIT_CONTROL)` to negotiate, `set_alternate_setting` to pick a bandwidth-matching alt, then submit isoc transfers via `rusb::DeviceHandle::read_isochronous` and reassemble UVC payload headers into whole frames. Feed the result through the existing MJPEG / YUYV decoders in `nokhwa-core`. **Blocked on Windows**: `usbvideo.sys` owns the interface, so rusb's `claim_interface` returns `NotSupported` and we cannot replace the driver without breaking MSMF — the UVC backend on Windows stays enumeration-only.
  - [ ] **Session 3 — Controls (Linux/macOS).** Map `KnownCameraControl` to UVC class requests (`UVC_GET_CUR` / `UVC_SET_CUR` / `UVC_GET_RANGE`) on the `VideoControl` interface's Processing Unit + Camera Terminal; implement `controls()` and `set_control()`. Same Windows blocker as 2b.
  - [ ] **Session 4** (optional): hook into `nokhwa::open()` dispatch for feature `input-uvc` so `CameraIndex::Index` picks a UVC device without the user constructing `UVCCaptureDevice` directly.
- [x] ~~Re-implement Network/IP camera backend~~ — **already supported**. The OpenCV backend accepts an IP/RTSP URL as `CameraIndex::String` and opens it via `VideoCapture::from_file`. The old deprecated `NetworkCamera` wrapper was removed intentionally in favour of this path. Documented on the feature table in `README.md`.
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
