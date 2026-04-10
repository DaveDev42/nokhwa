# TODO

## High Priority
- [ ] Replace `core-media-sys` / `core-video-sys` with direct FFI declarations — these pull in legacy `objc 0.2` and `metal 0.18` transitively
- [ ] Fix NV12 formats incorrectly mapped to YUYV in macOS bindings (`util.rs:119`)
- [ ] Fix `Closest` format selection using requested resolution instead of computed closest (`types.rs:177`)
- [ ] Fix ObjC `lockForConfiguration:` error pointer passed by value — never captures NSError (`device.rs:462`)
- [ ] Add `Drop` implementation for `AVCaptureVideoCallback` (delegate/queue leak)

## Medium Priority
- [ ] Investigate WASM/browser support (`wasm-bindgen` non-C enum limitation)
- [ ] Split `device.rs` further (1,728 lines) — separate format discovery from device control
- [ ] Consider `Arc<Buffer>` in `CallbackCamera` to avoid frame clone per callback

## Low Priority
- [ ] Add integration tests for each platform backend (currently only core unit tests)
- [ ] Document minimum supported Rust version (currently requires nightly)
