# TODO

## High Priority
- [x] ~~Fix `CallbackCamera` Drop panic~~ — Already fixed: `Drop` uses `let _ = self.stop_stream()` and `stop_stream()` uses `if let Ok(...)` for mutex access. No `unwrap()` in destructor path.
- [x] ~~Remove `unsafe impl Send for Camera` (`camera-sync-impl` feature)~~ — Done: replaced with `Send` bound on `Box<dyn CaptureBackendTrait + Send>` and `unsafe impl Send` on `AVFoundationCaptureDevice`.

## Medium Priority
- [ ] Add integration tests for each platform backend (currently only core unit tests + macOS format tests)
  - [ ] Format conversion correctness (MJPEG, NV12, YUYV decoding validation)
  - [ ] Camera control round-trip (set → get verification)
  - [ ] Robustness against malformed input (e.g. malformed MJPEG)
- [x] Restructure error types — replaced `String`-based variants (`GeneralError`, `OpenStreamError`, `ReadFrameError`, `StreamShutdownError`) with structured context (backend, format). Fixed `UnitializedError` → `UninitializedError` typo. Added helper constructors for backwards compatibility.
- [ ] Restructure `OpenDeviceError(String, String)` tuple variant with structured fields (deferred from error type restructuring)
- [x] ~~Replace `cap_impl_fn!`/`cap_impl_matches!` macros with a Backend Registry or Builder pattern~~ — Done in #43: replaced with explicit factory functions.
- [x] ~~Unify timestamp semantics~~ — Done: added `TimestampKind` enum (`Capture`, `Presentation`, `MonotonicClock`, `WallClock`, `Unknown`) paired with `Duration` in `Buffer`.

## Low Priority
- [ ] Add async support — `frame()` is blocking with no timeout mechanism. Short-term: add `frame_timeout(Duration)`. Long-term: consider `AsyncCaptureBackendTrait` or `Stream`-based API.
- [ ] Improve wgpu integration — `frame_texture()` always converts to RGBA regardless of input format. Provide raw texture variants so NV12/YUYV can be decoded via GPU shaders for better performance.

## Performance
- [ ] Eliminate redundant frame copies — Buffer::new() always uses `Bytes::copy_from_slice()`. Add `Buffer::from_vec(Vec<u8>)` constructor using `Bytes::from(vec)` for zero-copy ownership transfer. Update all backends to use it.
- [ ] Remove AVFoundation double copy — CMSampleBuffer → `to_vec()` → `Bytes::copy_from_slice()` causes 2 full copies per frame. Use `Bytes::from(vec)` for the second step; long-term, explore direct CMSampleBuffer → Bytes path.
- [x] ~~Optimize YUYV decoder~~ — Replaced `flat_map` + `flatten` with `chunks_exact` + `chunks_exact_mut` + `zip` direct buffer writes. Inlined YUV→RGB math, unified RGB/RGBA branches, eliminated bounds checks.
- [x] ~~Optimize NV12 decoder~~ — Done: pre-computed UV row offset and output row offset outside inner loop (#53). Consider further inlining `yuyv444_to_rgb` calls (same pattern as YUYV optimization).
- [ ] Explore `unsafe get_unchecked` for YUYV/NV12 inner loops — `chunks_exact` + `zip` eliminates chunk-boundary bounds checks, but within-chunk indexing (`out[pixel_size + 3]`) may still emit checks. Benchmark first; only apply if measurable gain.
- [ ] Reduce CallbackCamera lock contention — 3 sequential mutex acquisitions per frame (Camera, Callback, last_frame). Replace `last_frame` with lock-free pattern (e.g. `ArcSwap`), shrink lock scopes.
- [ ] Explore SIMD for pixel format conversion — YUYV/NV12/BGR→RGB decoders use scalar arithmetic. Consider `std::simd` (nightly) or manual intrinsics for 4-8x throughput.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
