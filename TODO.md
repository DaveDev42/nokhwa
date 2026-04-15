# TODO

## High Priority
(None)

## Medium Priority
(None)

## Low Priority
- [ ] Expand platform integration tests (requires physical camera, gated behind `device-test` feature). Port in `tests/device_tests.rs` covers query, multi-frame streaming, control enumeration, and `CameraRunner` smoke. Linux is now auto-covered on every PR via `v4l2loopback` (see `.github/workflows/v4l-loopback.yml`) — the V4L `nokhwa::open` dispatch regression is CI-guarded. Still missing:
  - [ ] Camera control round-trip on real hardware (set → get value verification). `v4l2loopback` exposes a limited control surface, so a dedicated hardware round-trip test is still pending on the self-hosted camera runner.
  - [ ] macOS / Windows virtual-camera story for GitHub-hosted runners (no clean equivalent of `v4l2loopback`). Currently only the self-hosted `macos-camera` runner covers AVFoundation; MSMF has no CI coverage.

## Performance
(None)

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
