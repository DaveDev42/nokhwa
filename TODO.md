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
- [x] ~~Eliminate redundant frame copies~~ — Done in #56: added `Buffer::from_vec()` and `Buffer::from_vec_with_timestamp()` zero-copy constructors using `Bytes::from(vec)`. Updated all backends to use them.
- [x] ~~Remove AVFoundation double copy~~ — Done in #52: eliminated second copy in AVFoundation frame capture pipeline.
- [x] ~~Optimize YUYV decoder~~ — Done in #58: SIMD-optimized (NEON on aarch64, scalar fallback on other platforms).
- [x] ~~Optimize NV12 decoder~~ — Done in #53: pre-computed UV row offset and output row offset outside inner loop.
- [x] ~~Reduce CallbackCamera lock contention~~ — Replaced `Mutex<Buffer>` last_frame with lock-free `ArcSwap`, reduced per-frame lock acquisitions from 3 to 1.
- [x] ~~Explore SIMD for pixel format conversion~~ — Done in #58: NEON intrinsics for YUYV/BGR→RGB on aarch64, SSSE3 for BGR→RGB on x86_64, scalar fallback for other architectures.
- [ ] Explore `unsafe get_unchecked` for YUYV/NV12 inner loops — SIMD fallback and NV12 scalar paths use safe indexing. Within-chunk indexing (`out[pixel_size + 3]`) may still emit bounds checks. Benchmark first; only apply if measurable gain.
- [ ] Inline `yuyv444_to_rgb` in NV12 decoder — same optimization pattern as YUYV (#58). The NV12 scalar path still calls the helper function with intermediate `[u8; 3]` array allocation per pixel.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
