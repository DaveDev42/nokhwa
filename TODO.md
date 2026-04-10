# TODO

## High Priority
- [ ] Re-enable WASM/browser support — needs fresh implementation. Requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.

## Medium Priority
- [ ] Add integration tests for each platform backend (currently only core unit tests + macOS format tests)

## Low Priority
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
