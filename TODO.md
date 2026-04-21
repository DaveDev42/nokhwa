# TODO

## High Priority
(None)

## Medium Priority
- [ ] **OpenCV backend: re-open path for `CameraIndex::String` (IP cameras).** The constructor in `src/backends/capture/opencv_backend.rs` builds an already-open `VideoCapture` via `VideoCapture::from_file(ip, api_pref)` at line 169, but `FrameSource::open()` at line 546 errors on `CameraIndex::String` with `"String index not supported (try NetworkCamera instead)"`. Consequences: (a) `close()` followed by `open()` breaks for IP cameras; (b) the error message references the removed `NetworkCamera` type. Likely fix: replace the `Err(...)` arm with a call through the Rust opencv crate's filename-overload open (either `VideoCapture::from_file` reconstructing `self.video_capture` or `open_file(&str, i32)` if the binding exposes it). Requires testing against an actual IP/RTSP stream — no hardware available on the current dev box.

## Low Priority
- [ ] Expand platform integration tests (requires physical camera, gated behind `device-test` feature). Port in `tests/device_tests.rs` covers query, multi-frame streaming, control enumeration, and `CameraRunner` smoke.
  - Linux is now auto-covered on every PR via `v4l2loopback` (see `.github/workflows/v4l-loopback.yml`) — the V4L `nokhwa::open` dispatch regression is CI-guarded.
  - [x] Camera control round-trip on real hardware (set → get value verification). `tests/device_tests.rs::control_set_get_round_trip` picks the first Manual-mode `IntegerRange` control with headroom, writes a stepped value, re-reads, and asserts. Verified on a Windows MSMF webcam (2026-04-20) against `Brightness` 128 → 129. Skips gracefully on devices that only expose Automatic/ReadOnly controls (e.g. `v4l2loopback`).
  - [ ] **MSMF** CI coverage on a GitHub-hosted `windows-latest` runner (spike). Investigation 2026-04-20 ruled out kernel/AVStream drivers (require test-signing) and Camera Extension samples (require signing cert). Practical path: `choco install -y obs-studio`, launch `obs64.exe --startvirtualcam` as a background process, then run `cargo test --features "input-msmf,device-test" --test device_tests`. OBS's virtual camera registers as an MSMF source on modern builds. `softcam` is a lighter alternative but its DShow-only filter may not surface through the Windows Camera Frame Server. MSMF manually verified on a Windows dev box 2026-04-20 (4/4 device tests pass).
  - [x] ~~macOS GH-hosted virtual camera~~ — **not feasible**. Modern virtual cameras require system extensions that must be codesigned + notarized + installed from `/Applications`, which GitHub-hosted macOS runners cannot supply (no Apple Developer credentials). AVFoundation CI coverage remains the responsibility of the self-hosted `macos-camera` runner.

## Performance
(None)

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously ~839 lines). Would live in a new `nokhwa-bindings-gstreamer` crate alongside the existing per-OS binding crates. Significant scope — not a one-session task.
- [ ] Re-implement UVC backend (cross-platform via libusb, previously ~561 lines). Needs a new `nokhwa-bindings-uvc` crate with `libusb`/`rusb` + UVC protocol decode. Significant scope.
- [x] ~~Re-implement Network/IP camera backend~~ — **already supported**. The OpenCV backend at `src/backends/capture/opencv_backend.rs:167-170` accepts an IP/RTSP URL as `CameraIndex::String` and opens it via `VideoCapture::from_file`. The old deprecated `NetworkCamera` wrapper was removed intentionally in favour of this path. Documented on the feature table in `README.md`. (A related re-open bug is tracked under Medium Priority.)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
