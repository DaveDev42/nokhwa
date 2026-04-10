# TODO

## High Priority
- [ ] Re-enable WASM/browser support — `js_camera.rs` was removed, needs fresh implementation. Requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen` as alternatives.
- [ ] Migrate from raw `objc2::msg_send!` to typed `objc2-av-foundation` wrappers — would eliminate ~132 unsafe msg_send! calls and ~176 unsafe blocks in macOS bindings. Crate exists at v0.3.2 with full API coverage (AVCaptureDevice, AVCaptureSession, etc.). Also use `objc2-core-media` and `objc2-core-video` for typed CoreMedia/CoreVideo APIs.

## Medium Priority
- [ ] Split `device.rs` further (1,728 lines) — separate format discovery from device control
- [ ] Add integration tests for each platform backend (currently only core unit tests)

## Low Priority
- [ ] Investigate replacing `flume` with `std::sync::mpsc` or `crossbeam-channel` — currently only 3 unbounded channel usages, low priority
