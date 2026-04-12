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
(None)

## 0.13.0 Roadmap
- [ ] Separate streaming vs still-image capture models in `CaptureBackendTrait`
  - Current trait assumes continuous streaming (`open_stream` → `frame` → `stop_stream`). Does not fit cameras with distinct live-view + shutter-capture modes.
  - Split into `StreamBackend` (live view / continuous frames) and `CaptureBackend` (single-shot still images). Backends can implement one or both.
  - Enables proper support for DSLR/mirrorless SDKs (Canon EDSDK, Nikon SDK, Sony Remote SDK, gPhoto2), industrial cameras (Basler Pylon, Allied Vision Vimba, FLIR/Teledyne), and mobile camera APIs (Android Camera2, iOS AVCapturePhotoOutput).
  - Requires new API for high-resolution still capture, possibly RAW `FrameFormat` variants, and event-driven capture (trigger, shutter release).

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
