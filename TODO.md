# TODO

## High Priority
(None)

## Medium Priority
- [ ] Add MJPEG unit tests — needs a valid JPEG byte fixture to test `Frame<Mjpeg>.into_rgb().materialize()` correctness. Also add malformed MJPEG robustness tests (truncated, garbage data).
- [ ] Port OpenCV Mat methods to new Frame<F> API (removed Buffer::decode_opencv_mat in 0.12.0)

## Low Priority
- [ ] Add platform integration tests (requires physical camera, gated behind `device-test` feature)
  - [ ] End-to-end capture pipeline: format negotiation → stream open → frame capture → decode
  - [ ] Camera control round-trip on real hardware (set → get value verification)
  - [ ] Multi-frame streaming consistency (no corruption across frames)

## Low Priority
(None)

## Performance
(None)

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
