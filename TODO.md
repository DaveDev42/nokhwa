# TODO

## High Priority
- [x] ~~Improve API ergonomics: rename `camera_controls_string()` → `camera_controls_by_name()`, `camera_controls_known_camera_controls()` → `camera_controls_by_id()`, add convenience constructors, fix 'fufill' typo~~

## Medium Priority
- [ ] Add integration tests for each platform backend (currently only core unit tests + macOS format tests)
  - [ ] Format conversion correctness (MJPEG, NV12, YUYV decoding validation)
  - [ ] Camera control round-trip (set → get verification)
  - [ ] Robustness against malformed input (e.g. malformed MJPEG)

## Low Priority
(None)

## Performance
- [ ] Explore `unsafe get_unchecked` for YUYV scalar inner loops — NV12 done in #70, YUYV scalar fallback path still uses safe indexing.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
