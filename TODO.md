# TODO

## High Priority
- [ ] Re-enable WASM/browser support — `js_camera.rs` was removed, needs fresh implementation. Requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen` as alternatives.
- [ ] Migrate from raw `objc2::msg_send!` to typed `objc2-av-foundation` v0.3.2 wrappers — would eliminate ~132 unsafe msg_send! calls. Also use `objc2-core-media` and `objc2-core-video` for typed CoreMedia/CoreVideo APIs. Full API coverage confirmed.

## Medium Priority
- [ ] Split `device.rs` further (~1,500 lines) — separate format discovery from device control
- [ ] Add integration tests for each platform backend (currently only core unit tests)
- [ ] Version number cleanup — all crates still show pre-fork versions (root 0.10.10, core 0.1.8, bindings 0.2.3/0.1.3/0.4.5)

## Low Priority
- [ ] Investigate replacing `flume` with `std::sync::mpsc` or `crossbeam-channel` — currently only 3 unbounded channel usages, Bytes clone is already O(1), low priority
