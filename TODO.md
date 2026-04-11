# TODO

## High Priority
(None)

## Medium Priority
(None)

## Low Priority
- [ ] Add platform integration tests (requires physical camera, gated behind `device-test` feature)
  - [ ] End-to-end capture pipeline: format negotiation → stream open → frame capture → decode
  - [ ] Camera control round-trip on real hardware (set → get value verification)
  - [ ] Multi-frame streaming consistency (no corruption across frames)

## Performance
- [ ] Add SIMD NV12→RGB/RGBA decoder — NV12 currently scalar-only, biggest remaining SIMD gap. NEON (aarch64) + SSE2/AVX2 (x86_64).
- [ ] Add YUYV→RGB/RGBA SIMD for x86_64 — NEON path exists but x86_64 falls back to scalar. Add SSE2 or AVX2 path.
- [ ] Add AVX2 path for BGR→RGB on x86_64 — current SSSE3 processes 16 bytes/iter, AVX2 can do 32.
- [ ] Add SIMD RAWRGB→RGBA / RAWBGR→RGBA — 3-byte to 4-byte expansion with alpha insertion. NEON + SSE2/AVX2.
- [ ] Add SIMD YUYV Y-channel extraction — stride-2 byte extraction via NEON `vuzp` / x86 `pshufb`. Currently scalar.
- [ ] Add SIMD RGB→Luma averaging — 3-channel weighted/simple average. NEON + SSE2. Used by RAWRGB/RAWBGR→Luma path.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
