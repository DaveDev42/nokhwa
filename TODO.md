# TODO

## High Priority
(None)

## Medium Priority
- [ ] Add integration tests for each platform backend (currently only core unit tests + macOS format tests)
  - [ ] Format conversion correctness (MJPEG, NV12, YUYV decoding validation)
  - [ ] Camera control round-trip (set → get verification)
  - [ ] Robustness against malformed input (e.g. malformed MJPEG)
- [ ] Restructure `OpenDeviceError(String, String)` tuple variant with structured fields

## Low Priority
(None)

## Performance
- [ ] Explore `unsafe get_unchecked` for YUYV/NV12 inner loops — SIMD fallback and NV12 scalar paths use safe indexing. Benchmark first; only apply if measurable gain.
- [ ] Inline `yuyv444_to_rgb` in NV12 decoder — same optimization pattern as YUYV (#58). The NV12 scalar path still calls the helper function with intermediate `[u8; 3]` array allocation per pixel.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
