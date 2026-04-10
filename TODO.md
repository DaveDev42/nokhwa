# TODO

## High Priority
- [x] ~~Fix `CallbackCamera` Drop panic~~ — Already fixed: `Drop` uses `let _ = self.stop_stream()` and `stop_stream()` uses `if let Ok(...)` for mutex access. No `unwrap()` in destructor path.
- [ ] Remove `unsafe impl Send for Camera` (`camera-sync-impl` feature) — backend types should satisfy `Send` at the type level, or `Camera` should be made generic to enforce it at compile time.

## Medium Priority
- [ ] Add integration tests for each platform backend (currently only core unit tests + macOS format tests)
  - [ ] Format conversion correctness (MJPEG, NV12, YUYV decoding validation)
  - [ ] Camera control round-trip (set → get verification)
  - [ ] Robustness against malformed input (e.g. malformed MJPEG)
- [ ] Restructure error types — replace `String`-based variants (`GeneralError`, `OpenStreamError`, `ReadFrameError`) with structured context (backend, attempted format, device index). Also fix `UnitializedError` typo.
- [ ] Replace `cap_impl_fn!`/`cap_impl_matches!` macros with a Backend Registry or Builder pattern — current macro-based backend dispatch in `camera.rs` is hard to read and debug.
- [ ] Unify timestamp semantics — `Buffer::capture_timestamp` (`Option<Duration>`) has different meanings per platform (macOS: presentation TS, Linux: v4l2 TS, Windows: COM clock). Introduce a `TimestampSource` enum to let consumers distinguish semantics.

## Low Priority
- [ ] Add async support — `frame()` is blocking with no timeout mechanism. Short-term: add `frame_timeout(Duration)`. Long-term: consider `AsyncCaptureBackendTrait` or `Stream`-based API.
- [ ] Remove deprecated `Camera::new_with()` (deprecated since 0.10.0) — or at minimum document a removal timeline.
- [ ] Improve wgpu integration — `frame_texture()` always converts to RGBA regardless of input format. Provide raw texture variants so NV12/YUYV can be decoded via GPU shaders for better performance.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
