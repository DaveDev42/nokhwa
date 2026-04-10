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
- [ ] Improve wgpu integration — `frame_texture()` always converts to RGBA regardless of input format. Provide raw texture variants so NV12/YUYV can be decoded via GPU shaders for better performance.

## Performance
- [ ] Eliminate redundant frame copies — Buffer::new() always uses `Bytes::copy_from_slice()`. Add `Buffer::from_vec(Vec<u8>)` constructor using `Bytes::from(vec)` for zero-copy ownership transfer. Update all backends to use it.
- [ ] Remove AVFoundation double copy — CMSampleBuffer → `to_vec()` → `Bytes::copy_from_slice()` causes 2 full copies per frame. Use `Bytes::from(vec)` for the second step; long-term, explore direct CMSampleBuffer → Bytes path.
- [ ] Optimize YUYV decoder — `types.rs` lines 1460-1536 use per-pixel `flat_map` + intermediate arrays. Replace with `chunks_exact` + direct write to eliminate iterator overhead.
- [ ] Optimize NV12 decoder — `types.rs` lines 1590-1664 repeat UV plane index division per pixel. Pre-compute row offsets and use direct indexing.
- [ ] Reduce CallbackCamera lock contention — 3 sequential mutex acquisitions per frame (Camera, Callback, last_frame). Replace `last_frame` with lock-free pattern (e.g. `ArcSwap`), shrink lock scopes.
- [ ] Explore SIMD for pixel format conversion — YUYV/NV12/BGR→RGB decoders use scalar arithmetic. Consider `std::simd` (nightly) or manual intrinsics for 4-8x throughput.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
