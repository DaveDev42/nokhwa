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

## Performance
(None)

## 0.14.0 Roadmap
- [ ] `AsyncCameraRunner` behind an `async-tokio` feature (tokio-based channels; replaces ad-hoc `spawn_blocking` wrapping of `recv`).
- [ ] Migrate `input-opencv` backend to the 0.13.0 trait split (currently gated behind a `compile_error!`).
- [ ] Wire bounded channels + `Overflow` policy in `CameraRunner`. The `RunnerConfig` fields (`frame_queue`, `picture_queue`, `event_queue`, `on_overflow`) currently land but don't affect channel capacity (runner uses `std::sync::mpsc::channel`, which is unbounded). Switch to `sync_channel` with an explicit drop-oldest / drop-newest helper.
- [ ] Reconsider `CameraSession` as a builder. After T13, the unit struct + static `open()` makes `CameraSession::new` + `self.request` vestigial; fold into a single `open(index, req)` free fn or a real builder.
- [ ] Port `tests/device_tests.rs` (gated `device-test`) to the new API. It still references the removed `Camera`/`CallbackCamera`.
- [ ] Fix the `docs-only + docs-nolink + input-msmf` stub export so `cargo doc --features docs-only,docs-nolink` builds on non-Windows hosts (MSMF crate's docs-only branch doesn't re-export `MediaFoundationCaptureDevice`).
- [ ] External backend crate (e.g. `canon-edsdk-nokhwa`) validating the shutter/hybrid contract.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
