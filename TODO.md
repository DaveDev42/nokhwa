# TODO

## High Priority
(None)

## Medium Priority
(None)

## Low Priority
- [ ] Expand platform integration tests (requires physical camera, gated behind `device-test` feature). Port in `tests/device_tests.rs` covers query, multi-frame streaming, control enumeration, and `CameraRunner` smoke.
  - Linux is now auto-covered on every PR via `v4l2loopback` (see `.github/workflows/v4l-loopback.yml`) — the V4L `nokhwa::open` dispatch regression is CI-guarded.
  - [x] Camera control round-trip on real hardware (set → get value verification). `tests/device_tests.rs::control_set_get_round_trip` picks the first Manual-mode `IntegerRange` control with headroom, writes a stepped value, re-reads, and asserts. Verified on a Windows MSMF webcam (2026-04-20) against `Brightness` 128 → 129. Skips gracefully on devices that only expose Automatic/ReadOnly controls (e.g. `v4l2loopback`).
  - [ ] **MSMF** CI coverage on a GitHub-hosted `windows-latest` runner (spike). Drafted in `.github/workflows/msmf-obs-virtualcam.yml` (trigger: `workflow_dispatch`-only, `continue-on-error: true`). Installs OBS Studio via chocolatey, launches `obs64.exe --startvirtualcam` as a background process, and runs `cargo test --features "input-msmf,device-test,runner" --test device_tests`. Open questions observable in the first workflow run: whether OBS's first-run setup needs a seeded profile/scene-collection, and whether the virtual camera surfaces as an MSMF source with no output source configured. Once the run is green, promote the trigger to `pull_request` alongside the V4L loopback job. MSMF manually verified on a Windows dev box 2026-04-20 (4/4 device tests pass).
  - [x] ~~macOS GH-hosted virtual camera~~ — **not feasible**. Modern virtual cameras require system extensions that must be codesigned + notarized + installed from `/Applications`, which GitHub-hosted macOS runners cannot supply (no Apple Developer credentials). AVFoundation CI coverage remains the responsibility of the self-hosted `macos-camera` runner.

## Performance
(None)

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously ~839 lines). Would live in a new `nokhwa-bindings-gstreamer` crate alongside the existing per-OS binding crates. Significant scope — not a one-session task.
- [ ] Re-implement UVC backend (cross-platform via libusb, previously ~561 lines). Multi-session rollout in progress:
  - [x] **Session 1** (2026-04-20, this release): New `nokhwa-bindings-uvc` workspace crate using `rusb`. `query(ApiBackend::UniversalVideoClass)` enumerates UVC devices by matching `bInterfaceClass = 0x0E` / `bInterfaceSubClass = 0x01`, populating `CameraInfo` with iProduct / iManufacturer strings and `"<bus>:<addr> <vid>:<pid>"` in `misc`. `UVCCaptureDevice::new()` and every `FrameSource` method currently error with `NotImplementedError` — the skeleton lets the `input-uvc` feature flag, `nokhwa_backend!` registration, and CI coverage land ahead of the streaming work. CI: `.github/workflows/test-core.yml::check-uvc`. Verified on Windows against a Logitech MX Brio (VID 046d:0944).
  - [ ] **Session 2**: Streaming. Implement `UVCCaptureDevice::new()` (open the first VideoStreaming interface, select alt-setting, negotiate probe/commit), `FrameSource::open/frame/close`, and format enumeration via `compatible_formats()`. Feed frames through the existing MJPEG / YUYV decoders in `nokhwa-core`.
  - [ ] **Session 3**: Controls. Map `KnownCameraControl` to UVC `UVC_SET_CUR` / `UVC_GET_CUR` / `UVC_GET_RANGE` class requests; implement `controls()` and `set_control()`.
  - [ ] **Session 4** (optional): hook into `nokhwa::open()` dispatch for feature `input-uvc` so `CameraIndex::Index` picks a UVC device without the user having to construct `UVCCaptureDevice` directly.
- [x] ~~Re-implement Network/IP camera backend~~ — **already supported**. The OpenCV backend accepts an IP/RTSP URL as `CameraIndex::String` and opens it via `VideoCapture::from_file`. The old deprecated `NetworkCamera` wrapper was removed intentionally in favour of this path. Documented on the feature table in `README.md`.
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
