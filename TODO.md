# TODO

## High Priority
(None)

## Medium Priority
(None)

## Low Priority
- [ ] Add platform integration tests (requires physical camera, gated behind `device-test` feature)
  - [ ] End-to-end capture pipeline: format negotiation → stream open → frame capture → decode
  - [ ] Camera control round-trip on real hardware (set → get value verification)
  - [ ] Multi-frame streaming consistency (no corruption across frames)
  - [ ] V4L `CameraSession::open` dispatch regression test (hardware-gated) — guards against the 0.13.0 stub recurring silently

## Performance
(None)

## 0.14.0 Roadmap
- [ ] Migrate `input-opencv` backend to the 0.13.0 trait split (currently gated behind a `compile_error!`).
- [ ] Reconsider `CameraSession` as a real builder or free `open()` function. 0.13.0 leaves it as a unit-struct namespace around `open()`.
- [ ] Port `tests/device_tests.rs` (gated `device-test`) to the new API. It still references the removed `Camera`/`CallbackCamera`.
- [ ] Restore a ggez-based live-view demo in `examples/capture` (lost in the 0.13.0 refactor).
- [ ] Fix the `docs-only + docs-nolink + input-msmf` stub export so `cargo doc --features docs-only,docs-nolink` builds on non-Windows hosts (MSMF crate's docs-only branch doesn't re-export `MediaFoundationCaptureDevice`).
- [ ] External backend crate (e.g. `canon-edsdk-nokhwa`) validating the shutter/hybrid contract.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
