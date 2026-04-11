# TODO

## High Priority
- [x] Fix release-please config to prevent premature 1.0.0 bumps on breaking changes (`bump-major-pre-major: false`)

## Medium Priority
- [x] Add MJPEG unit tests — Frame<Mjpeg> into_rgb/into_rgba/into_luma + write_to + malformed/empty error cases added
- [x] Port OpenCV Mat methods to new Frame<F> API (removed Buffer::decode_opencv_mat in 0.12.0)

## Low Priority
- [ ] Add platform integration tests (requires physical camera, gated behind `device-test` feature)
  - [ ] End-to-end capture pipeline: format negotiation → stream open → frame capture → decode
  - [ ] Camera control round-trip on real hardware (set → get value verification)
  - [ ] Multi-frame streaming consistency (no corruption across frames)

## Performance
- [x] Add SIMD NV12→RGB/RGBA decoder — NEON (aarch64) + SSE4.1 (x86_64)
- [x] Add YUYV→RGB/RGBA SIMD for x86_64 — SSE4.1 path added alongside existing NEON
- [x] Add AVX2 path for BGR→RGB on x86_64 — 30 bytes/iter with AVX2 → SSSE3 → scalar fallback
- [x] Add SIMD RAWRGB→RGBA / RAWBGR→RGBA — NEON vld3q/vst4q + SSSE3 pshufb expansion
- [x] Add SIMD YUYV Y-channel extraction — NEON vld2q deinterleave + SSSE3 pshufb
- [x] Add SIMD RGB→Luma averaging — NEON + SSE2 with multiply-high division trick

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
