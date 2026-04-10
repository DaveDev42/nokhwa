# TODO

## High Priority
- [ ] Re-enable WASM/browser support — `js_camera.rs` was removed, needs fresh implementation. Requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen` as alternatives.

## Medium Priority
- [ ] Version number cleanup — bump all crates to unified `0.11.0`
- [ ] Add integration tests for each platform backend (currently only core unit tests + macOS format tests)

## Low Priority
- [ ] Re-implement GStreamer backend (`gst_backend.rs` removed — was 839 lines, disabled)
- [ ] Re-implement UVC backend (`uvc_backend.rs` removed — was 561 lines, disabled since soundness concerns)
- [ ] Re-implement Network/IP camera backend (`network_camera.rs` removed — was 173 lines, disabled)
