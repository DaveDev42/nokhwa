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
  - [ ] V4L `nokhwa::open` dispatch regression test (hardware-gated) — guards against the 0.13.0 stub recurring silently

## Performance
- [ ] `OpenCvCaptureDevice::raw_frame_vec` allocates a fresh `Vec<u8>` per frame and swizzles BGR→RGB byte-by-byte. Rewrite as a single pre-allocated buffer with chunked swap for throughput.

## 0.14.0 Roadmap
- [ ] Wire `input-opencv` into CI (requires installing the OpenCV system library on the runner).

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
