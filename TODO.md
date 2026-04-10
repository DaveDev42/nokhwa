# TODO

## High Priority
- [ ] Re-enable WASM/browser support — `js_camera.rs` (2,715 lines) exists but is disabled behind removed `input-jscam` feature. Requires resolving `wasm-bindgen` non-C enum limitation (see CHANGELOG 0.10.0). Consider using `tsify` or `serde-wasm-bindgen` as alternatives.
- [x] Replace `core-media-sys` / `core-video-sys` with direct FFI declarations — these pull in legacy `objc 0.2` and `metal 0.18` transitively, conflicting with our `objc2` migration
- [x] Fix NV12 formats incorrectly mapped to YUYV in macOS bindings (`util.rs:119-121`)
- [ ] Fix `Closest` format selection using requested resolution instead of computed closest (`types.rs:174-182`)
- [x] Fix ObjC `lockForConfiguration:` error pointer passed by value — never captures NSError (`device.rs:~465`)
- [x] Add `Drop` implementation for `AVCaptureVideoCallback` (delegate/queue memory leak)

## Medium Priority
- [ ] Split `device.rs` further (1,728 lines) — separate format discovery from device control
- [ ] Consider `Arc<Buffer>` in `CallbackCamera` to avoid frame clone per callback invocation
- [ ] Add integration tests for each platform backend (currently only core unit tests)

## Low Priority
- [ ] Document minimum supported Rust version (currently requires nightly)
- [ ] Investigate replacing `flume` with `std::sync::mpsc` or `crossbeam-channel` to reduce dependencies
