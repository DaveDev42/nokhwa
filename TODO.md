# TODO

## High Priority
- [ ] Type-safe decode API (0.12.0 breaking change)
  - [ ] `CaptureFormat` marker types (Yuyv, Nv12, Mjpeg, Gray, RawRgb, RawBgr) in nokhwa-core
  - [ ] Typed `Camera<F: CaptureFormat>` with `Camera::open::<F>()` constructor
  - [ ] Lazy `Frame<F>` type — holds raw data, no conversion until materialized
  - [ ] Conversion traits: `IntoRgb`, `IntoRgba`, `IntoLuma` — each Frame type only implements valid conversions
  - [ ] `RgbConversion`/`RgbaConversion`/`LumaConversion` with `materialize()` → `ImageBuffer` and `write_to(&mut [u8])`
  - [ ] YUYV/NV12→Luma: direct Y channel extraction instead of RGB→avg
  - [ ] GRAY→RGB: compile error (Frame\<Gray\> does not implement IntoRgb)
  - [ ] Remove `FormatDecoder` trait and old decode API
  - [ ] Revert `image` crate to required dependency (#81 revert)
  - [ ] Version bump to 0.12.0

## Medium Priority
- [ ] Add MJPEG positive correctness unit test — currently only error cases tested (empty, truncated, garbage). Need a valid JPEG byte fixture to verify `mjpeg_to_rgb()` produces correct RGB output.

## Low Priority
- [ ] Add platform integration tests (requires physical camera, gated behind `device-test` feature)
  - [ ] End-to-end capture pipeline: format negotiation → stream open → frame capture → decode
  - [ ] Camera control round-trip on real hardware (set → get value verification)
  - [ ] Multi-frame streaming consistency (no corruption across frames)

## Performance
- [ ] Add SIMD NV12→RGB/RGBA decoder — NV12 currently scalar-only, biggest remaining SIMD gap. NEON (aarch64) + SSE2 (x86_64). Expected 5-8x speedup.
- [ ] Replace iterator patterns in pixel_format.rs with direct loops — `flat_map().collect()` and `.enumerate().for_each()` add overhead at 1080p+. (May be superseded by type-safe decode API rewrite.)
- [ ] Add AVX2 path for BGR→RGB on x86_64 — current SSSE3 processes 16 bytes/iter, AVX2 can do 32.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
