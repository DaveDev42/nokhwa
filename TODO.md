# TODO

## High Priority
(None)

## Medium Priority
- [x] Extract common backend logic into nokhwa-core (FourCC helpers, control index helpers, normalized query function names)
- [ ] Add integration tests for each platform backend (currently only core unit tests + macOS format tests)
  - [ ] Format conversion correctness (MJPEG, NV12, YUYV decoding validation)
  - [ ] Camera control round-trip (set → get verification)
  - [ ] Robustness against malformed input (e.g. malformed MJPEG)

## Low Priority
(None)

## Performance
(None)

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
