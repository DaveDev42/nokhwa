# TODO

## High Priority
- [ ] Add structured logging — replace `dbg!()` in `query.rs` (5 instances) and `eprintln!()` in `threaded.rs` with `log` or `tracing` crate behind an optional feature flag.

## Medium Priority
- [ ] Add integration tests for each platform backend (currently only core unit tests + macOS format tests)
  - [ ] Format conversion correctness (MJPEG, NV12, YUYV decoding validation)
  - [ ] Camera control round-trip (set → get verification)
  - [ ] Robustness against malformed input (e.g. malformed MJPEG)
- [ ] Improve API ergonomics
  - [ ] Rename confusing camera control methods: `camera_controls_string()` → `camera_controls_by_name()`, `camera_controls_known_camera_controls()` → `camera_controls_by_id()`
  - [ ] Add convenience constructors: `Camera::new_with_highest_resolution()`, `Camera::new_with_highest_framerate()`
  - [ ] Fix "fufill" typo in `set_camera_request()` error message → "fulfill"
- [ ] Improve documentation
  - [ ] Add proper doc comments to `Camera` struct (currently 3 lines) and `lib.rs` module docs (currently just "read README")
  - [ ] Add usage examples to `RequestedFormat`, `CaptureBackendTrait` method docs
  - [ ] Rewrite `examples/capture/README.md` (currently "use --help lol") with actual usage guide
- [ ] Extract common backend logic to `nokhwa-core` — control ID → `KnownCameraControl` mapping, format FourCC conversion, and query-to-`CameraInfo` patterns are duplicated across all three bindings crates. Normalize query function names (`query()` vs `query_media_foundation_descriptors()`).

## Low Priority
- [ ] Remove `once_cell` dependency in Windows bindings — replace with `std::sync::OnceLock` (stable since Rust 1.80)
- [ ] Make `image` crate dependency optional — currently pulled in even when decoding is not used
- [ ] Improve feature flag discoverability
  - [ ] Add compile-time warning when no `input-*` feature is enabled (library is non-functional without one)
  - [ ] Consider renaming `input-native` to `input-auto` for clarity
  - [ ] Document feature dependencies in `Cargo.toml` comments

## Performance
- [ ] Explore `unsafe get_unchecked` for YUYV scalar inner loops — NV12 done in #70, YUYV scalar fallback path still uses safe indexing.

## Backlog
- [ ] Re-implement GStreamer backend (cross-platform, previously 839 lines)
- [ ] Re-implement UVC backend (cross-platform via libusb, previously 561 lines)
- [ ] Re-implement Network/IP camera backend (previously 173 lines)
- [ ] Re-implement WASM/browser backend — requires resolving `wasm-bindgen` non-C enum limitation. Consider `tsify` or `serde-wasm-bindgen`.
