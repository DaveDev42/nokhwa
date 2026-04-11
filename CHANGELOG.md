# Changelog

## [1.0.0](https://github.com/DaveDev42/nokhwa/compare/v0.11.1...v1.0.0) (2026-04-11)


### ⚠ BREAKING CHANGES

* type-safe decode API (0.12.0) ([#85](https://github.com/DaveDev42/nokhwa/issues/85))

### Features

* type-safe decode API (0.12.0) ([#85](https://github.com/DaveDev42/nokhwa/issues/85)) ([6874fb6](https://github.com/DaveDev42/nokhwa/commit/6874fb6b20cdd282a487977f345653df88a87408))

## 0.12.0 (unreleased)

### Breaking Changes

* **Type-safe decode API**: `Camera` and `CallbackCamera` are now generic over `CaptureFormat` (`Camera<F: CaptureFormat = Mjpeg>`). New `Camera::open::<F>()` constructor selects format at compile time.
* **Removed `FormatDecoder` trait and `pixel_format.rs`**: The old `Buffer::decode_image::<RgbFormat>()` pattern is replaced by `Frame<F>` with `IntoRgb`, `IntoRgba`, and `IntoLuma` conversion traits.
* **Removed `decoding` feature flag**: The `image` crate is now always required. MJPEG decoding is controlled by the `mjpeg` feature (enabled by default).
* **`ApiBackend::Custom(String)` variant added**: `ApiBackend` is no longer `Copy` (now `Clone` only). `Camera::backend()` returns `&ApiBackend`.
* **`Buffer` API reduced**: Removed `decode_image()`, `decode_image_to_buffer()`, `decode_opencv_mat()`, `decode_into_opencv_mat()` methods. Use `Frame<F>` conversions instead.
* **`RequestedFormat::new::<F>()`** now takes a `CaptureFormat` type instead of `FormatDecoder`.

### Features

* **`CaptureFormat` trait + 6 marker ZSTs**: `Yuyv`, `Nv12`, `Mjpeg`, `Gray`, `RawRgb`, `RawBgr` in `nokhwa_core::format_types`.
* **`Frame<F>` typed frame handle**: Lazy conversion via `into_rgb()`, `into_rgba()`, `into_luma()` returning `RgbConversion`, `RgbaConversion`, `LumaConversion` structs. `materialize()` performs the actual pixel conversion.
* **Compile-time format safety**: `Frame<Gray>` does not implement `IntoRgb` or `IntoRgba` — attempting grayscale-to-RGB conversion is a compile error.
* **Direct Y-channel extraction**: `buf_yuyv_extract_luma()` and `buf_nv12_extract_luma()` extract luminance without intermediate RGB conversion.
* **`Camera::frame_typed()`**: Returns `Frame<F>` for type-checked frame capture.

### Bug Fixes

* **wgpu**: Fixed `frame_texture()` writing to `mip_level: 1` instead of `mip_level: 0` (base level).

## [0.11.1](https://github.com/DaveDev42/nokhwa/compare/v0.11.0...v0.11.1) (2026-04-11)


### Features

* add structured logging behind optional feature flag ([#76](https://github.com/DaveDev42/nokhwa/issues/76)) ([485eebc](https://github.com/DaveDev42/nokhwa/commit/485eebcc2397cceb6f2d2c95e6b9ddaecef85d8b))


### Bug Fixes

* **ci:** switch release-please to simple type for workspace compatibility ([#63](https://github.com/DaveDev42/nokhwa/issues/63)) ([4bd8243](https://github.com/DaveDev42/nokhwa/commit/4bd8243eda4ae623c743822ad20f91fdd1dab11a))


### Performance

* inline YUV-to-RGB conversion in NV12 decoder ([#67](https://github.com/DaveDev42/nokhwa/issues/67)) ([576e19b](https://github.com/DaveDev42/nokhwa/commit/576e19b09baeabf0fa149240b14fbc8302c74f3a))
* use unchecked indexing in NV12 scalar decoder hot loops ([#70](https://github.com/DaveDev42/nokhwa/issues/70)) ([031dfe2](https://github.com/DaveDev42/nokhwa/commit/031dfe2b095f9ad1bd0e0e2e5e7f2eb5fd213f73))
* use unchecked indexing in YUYV scalar decoder hot loops ([#73](https://github.com/DaveDev42/nokhwa/issues/73)) ([73576aa](https://github.com/DaveDev42/nokhwa/commit/73576aab676b4aa88a5d231ae869821b9fe12270))
* use unchecked indexing in YUYV/NV12 scalar decoder hot loops ([#69](https://github.com/DaveDev42/nokhwa/issues/69)) ([52cfa3f](https://github.com/DaveDev42/nokhwa/commit/52cfa3feb406bc1beca34bd6b6108021b1eb075d))


### Refactoring

* extract common backend logic to nokhwa-core, normalize query function names ([#80](https://github.com/DaveDev42/nokhwa/issues/80)) ([1adddeb](https://github.com/DaveDev42/nokhwa/commit/1adddeb767e83966ec03d8038910d0aa8069bbfa))
* improve Camera API ergonomics ([#77](https://github.com/DaveDev42/nokhwa/issues/77)) ([2b9a4d7](https://github.com/DaveDev42/nokhwa/commit/2b9a4d706e7992598f4a2c769bd63719945804b2))
* **macos:** reduce unsafe surface area with safe wrapper methods ([#78](https://github.com/DaveDev42/nokhwa/issues/78)) ([5b834f7](https://github.com/DaveDev42/nokhwa/commit/5b834f79634787382eb6cc4adf0d19e0eda048e2))
* restructure OpenDeviceError with named fields ([#66](https://github.com/DaveDev42/nokhwa/issues/66)) ([21c1471](https://github.com/DaveDev42/nokhwa/commit/21c1471d9b808e559b397688d8229107836ac8a1))


### Infrastructure

* gitignore Claude Code runtime files ([#82](https://github.com/DaveDev42/nokhwa/issues/82)) ([7108742](https://github.com/DaveDev42/nokhwa/commit/7108742987709041431b7e27aea3c8b2ee0e8391))
* improve feature flag discoverability with compile-time checks and documentation ([#74](https://github.com/DaveDev42/nokhwa/issues/74)) ([5af7c79](https://github.com/DaveDev42/nokhwa/commit/5af7c798dcb0a3af671a01d3aa6aa3b81ba1e29c))
* make image crate dependency optional, gated behind decoding feature ([#81](https://github.com/DaveDev42/nokhwa/issues/81)) ([addbe44](https://github.com/DaveDev42/nokhwa/commit/addbe4495f4c7f0fa3cae8a3143a2c3b24bc4a3c))
* replace once_cell with std::sync::LazyLock ([#75](https://github.com/DaveDev42/nokhwa/issues/75)) ([f436290](https://github.com/DaveDev42/nokhwa/commit/f43629095b582950531fb154290a6ad69f992da3))
* set up release-please for automated patch versioning ([#62](https://github.com/DaveDev42/nokhwa/issues/62)) ([3f606f1](https://github.com/DaveDev42/nokhwa/commit/3f606f1abcd674ecfefafaefe595fb1ab83a3d38))


### Documentation

* add new improvement items to TODO.md, add gw TODO rule to CLAUDE.md ([#72](https://github.com/DaveDev42/nokhwa/issues/72)) ([e5c1a7f](https://github.com/DaveDev42/nokhwa/commit/e5c1a7f65f3a16047be931600aa8eae26d5222c8))
* clean up TODO.md — remove all completed items from recent PRs ([#83](https://github.com/DaveDev42/nokhwa/issues/83)) ([11427f3](https://github.com/DaveDev42/nokhwa/commit/11427f32545a3cfa8332b37d49de9509e9938dd6))
* improve Camera, lib.rs, RequestedFormat, CaptureBackendTrait, and examples documentation ([#79](https://github.com/DaveDev42/nokhwa/issues/79)) ([6f5bf2d](https://github.com/DaveDev42/nokhwa/commit/6f5bf2d6249e1369575435ff9c4dda291cd05a2a))
* remove completed OpenDeviceError and NV12 inline items from TODO.md ([#68](https://github.com/DaveDev42/nokhwa/issues/68)) ([fe8e1d4](https://github.com/DaveDev42/nokhwa/commit/fe8e1d4ee33d681dda3a4f00af1e335a09d40a66))
* update TODO.md — reflect NV12 unchecked indexing done in [#70](https://github.com/DaveDev42/nokhwa/issues/70) ([#71](https://github.com/DaveDev42/nokhwa/issues/71)) ([f3c5a2e](https://github.com/DaveDev42/nokhwa/commit/f3c5a2e32f066d9f4d80d692011e6f8d91ce4bc0))
* update TODO.md — remove completed items, clean up stale entries ([#65](https://github.com/DaveDev42/nokhwa/issues/65)) ([3c6f5bb](https://github.com/DaveDev42/nokhwa/commit/3c6f5bb485c0491d6d44148754e76135c0c4fb2f))

## 0.11.0 (unreleased, fork: DaveDev42/nokhwa)

## Breaking
- Removed deprecated `Camera::new_with()` (use `Camera::new()` or `Camera::with_backend()` instead)
- Removed deprecated `Camera::set_camera_format()` and `CallbackCamera::set_camera_format()` (use `set_camera_request()` instead)
- Renamed bindings crates: `nokhwa-bindings-macos` → `nokhwa-bindings-macos-avfoundation`, `nokhwa-bindings-windows` → `nokhwa-bindings-windows-msmf`, `nokhwa-bindings-linux` → `nokhwa-bindings-linux-v4l`
- Unified all workspace crate versions to `0.11.0` (workspace version inheritance)
- Moved `CaptureBackendTrait` impl from root crate wrappers into bindings crates (consistent with Linux pattern)
- Replaced `flume` with `std::sync::mpsc` (API compatible but different error types)
- Removed `camera-sync-impl` feature flag — `Camera` is now `Send` at the type level via `Box<dyn CaptureBackendTrait + Send>`. The `output-threaded` feature no longer pulls in `camera-sync-impl`. `Camera::with_custom()` now requires `Box<dyn CaptureBackendTrait + Send>` (callers passing a non-Send backend will get a compile error).
- `NokhwaError` variant changes: `UnitializedError` renamed to `UninitializedError`; `GeneralError(String)`, `OpenStreamError(String)`, `ReadFrameError(String)`, `StreamShutdownError(String)` changed from tuple to struct variants with structured context fields

## Performance
- Optimized NV12 decoder: pre-computed UV row offset and output row offset outside inner loop, consolidated UV indexing to eliminate redundant per-pixel division
- Inlined BT.601 YUV-to-RGB conversion in NV12 scalar decoder, eliminating per-pixel helper function calls and intermediate array allocations
- CallbackCamera threading overhaul: eliminated simultaneous multi-lock, fixed memory ordering (SeqCst → Release/Acquire), added thread join in Drop
- CallbackCamera: replaced `Mutex<Buffer>` last_frame with lock-free `ArcSwap`, reducing per-frame lock acquisitions from 3 to 1
- Replaced `to_vec()` + sort allocations with zero-allocation `max_by_key` iterators in `RequestedFormat::fulfill()`
- Deduplicated Windows Media Foundation format enumeration (~80 lines removed)
- Removed unnecessary `Vec::default()` allocations in CallbackCamera

## Refactoring
- **Extracted common backend logic into nokhwa-core**: added `FrameFormat::from_fourcc()`/`to_fourcc()` for canonical FourCC string mapping, `KnownCameraControl::as_index()`/`from_index()`/`from_platform_id()`/`to_platform_id()` for shared control-ID mapping via lookup tables. V4L2 backend updated to use shared helpers. Normalized query function names: `query_media_foundation_descriptors()` → `query()` in MSMF, `query_avfoundation()` → `query()` in AVFoundation.
- Renamed `camera_controls_string()` → `camera_controls_by_name()` and `camera_controls_known_camera_controls()` → `camera_controls_by_id()` on `Camera` and `CallbackCamera` (old names kept as `#[deprecated]` aliases)
- Fixed 'fufill' → 'fulfill' typo in `set_camera_request()` error message
- **Restructured error types**: replaced `String`-based variants (`GeneralError`, `OpenStreamError`, `ReadFrameError`, `StreamShutdownError`) with structured fields (`backend: Option<ApiBackend>`, `format: Option<FrameFormat>`). Binding crates now populate context. Added helper constructors for backwards compatibility.
- Fixed `UnitializedError` typo → `UninitializedError`
- **macOS: migrated from `objc`/`cocoa-foundation` to `objc2`/`block2`** — eliminated all 186 deprecation warnings, reduced dependencies from 6 to 3
- Split macOS bindings monolith (2,422 lines) into 6 focused modules (ffi, util, types, callback, device, session)
- Fixed UB: `from_raw_parts_mut` → `from_raw_parts` in CVPixelBuffer callback

## Features
- Added convenience constructors `Camera::new_with_highest_resolution()` and `Camera::new_with_highest_framerate()`
- Added optional structured logging behind `logging` feature flag — replaces `dbg!()`/`eprintln!()` with `log` crate (`log::warn!`, `log::error!`)
- Added sensor capture timestamp support across all backends (cherry-picked from upstream l1npengtul/nokhwa#234)
  - `Buffer::with_timestamp()` constructor and `Buffer::capture_timestamp()` accessor
  - macOS: `CMSampleBufferGetPresentationTimeStamp` → wall clock conversion
  - Linux: `v4l2_buffer.timestamp` → wall clock conversion
  - Windows: `IMFSample::GetSampleTime` → wall clock conversion
- Added `TimestampKind` enum for platform-aware timestamp semantics
  - Variants: `Capture`, `Presentation`, `MonotonicClock`, `WallClock`, `Unknown`
  - `Buffer::with_timestamp()` now accepts `Option<(Duration, TimestampKind)>`
  - New `Buffer::capture_timestamp_with_kind()` accessor; `capture_timestamp()` remains backward-compatible
  - Each backend tags its timestamps: macOS → `Presentation`, Linux → `WallClock`, Windows → `MonotonicClock`
  - `#[non_exhaustive]` for future extensibility; serde support behind `serialize` feature

## Bug Fixes
- Fixed NV12 pixel formats (420 biplanar YCbCr) incorrectly mapped to `FrameFormat::YUYV` instead of `FrameFormat::NV12` in macOS bindings
- Fixed `lockForConfiguration:` error pointer passed by value (NSError** must be pointer-to-pointer) — ObjC runtime could never write back errors
- Fixed NV12 output format requesting 10-bit variant instead of 8-bit in `AVCaptureVideoDataOutput::set_frame_format`
- Fixed `AVCaptureVideoCallback` leaking ObjC delegate and GCD dispatch queue (added `Drop` impl)
- Fixed `wanted_decoder` filter inconsistently applied in `HighestResolution`/`HighestFrameRate` format selection
- Fixed several macOS AVFoundation bugs discovered during objc2 migration:
  - `maxWhiteBalanceGain` read as wrong type (UB)
  - `BacklightComp` setter sending wrong selector
  - `Gain` setter extracting wrong value type
  - `TorchMode` inverted flag logic
  - `data_len()` sending unregistered selector (runtime crash)
  - `CGFloat` incorrectly defined as f32 on 64-bit (should be f64)
- Poisoned mutex errors now logged instead of silently swallowed in CallbackCamera

## Infrastructure
- Added cross-platform CI: lint, build-matrix, test-core, device-test workflows
- Added pre-commit hook (cargo fmt + clippy)
- Added 24 unit tests for nokhwa-core
- Clippy pedantic: 30 errors → 0
- Made `image` crate dependency optional in both `nokhwa` and `nokhwa-core`, gated behind the `decoding` feature flag. Building without `decoding` no longer pulls in the `image` crate, reducing compile times and dependency count for users who only need raw frame capture.

## Cleanup
- Replaced `flume` crate with `std::sync::mpsc` to reduce external dependencies (all channel usages migrated in library and examples)
- Replaced `core-media-sys` / `core-video-sys` crate dependencies with direct FFI declarations in `ffi.rs`, eliminating legacy `objc 0.2` and `metal 0.18` transitive dependencies
- Removed unused dependencies from nokhwa-core: `usb_enumeration`, `regex`, `cocoa-foundation`, `core-foundation`, `once_cell`
- Replaced `once_cell::sync::Lazy` with `std::sync::LazyLock` in Windows bindings, removing `once_cell` dependency from `nokhwa-bindings-windows-msmf`
- Removed unused `once_cell` dependency from `nokhwactl` example
- Removed dead code: empty `VirtualBackendTrait`, commented-out module declarations, obsolete code blocks
- Removed obsolete `make-npm.sh` (JS bindings removed in 0.10.0)

## 0.10.0
- Split core types and traits into `nokhwa-core`
  - Now you can use `nokhwa`'s Camera types in your own packages, to e.g. create `nokhwa` extensions or use `nokhwa`'s decoders.  
- Removed support for JS Bindings
  - This is due to lack of support for non-C style enums in `wasm-bindgen`. 
  - You can still use `nokhwa` in the browser, you just can't use it from JS.
- New CameraControl API
  - Deprecated `raw_camera_control` API
- New RequestedFormat API
- Removed Network Camera 
  - Network Camera is now supported through OpenCV Camera instead.
- New Buffer API
- New PixelFormat API
- Callback Camera: Removed `Result` from the `index()` and `camera_info()` API.
- AVFoundation Improvements
- Split V4L2 into its own crate
- New Formats:
  - NV12
  - RAWRGB
  - GRAY
- Added warning about decoding on main thread reducing performance
- After a year in development, We hope it was worth the wait.

## 0.9.0
- Fixed Camera Controls for V4L2
- Disabled UVC Backend.
- Added polling and last frame to `ThreadedCamera`
- Updated the `CameraControl` related Camera APIs

## 0.8.0
- Media Foundation Access Violation fix (#13)

## 0.7.0
- Bumped some dependencies.

## 0.5.0
 - Fixed `msmf`
 - Relicensed to Apache-2.0

## 0.4.0
- Added AVFoundation, MSMF, WASM
- `.get_info()` returns a `&CameraInfo`
- Added Threaded Camera
- Added JSCamera
- Changed `new` to use `CaptureAPIBackend::Auto` by default. Old functionally still possible with `with_backend()`
- Added `query()`, which uses `CaptureAPIBackend::Auto` by default.
- Fixed/Added examples

## 0.3.2
- Bumped `ouroboros` to avoid potential UB
- [INTERNAL] Removed `Box<T>` from many internal struct fields of `UVCCaptureDevice`

## 0.3.1
- Added feature hacks to prevent gstreamer/opencv docs.rs build failure

## 0.3.0
- Added `query_devices()` to query available devices on system
- Added `GStreamer` and `OpenCV` backends
- Added `NetworkCamera`
- Added WGPU Texture and raw buffer write support
- Added `capture` example
- Removed `get_` from all APIs. 
- General documentation fixes
- General bugfixes/performance enhancements


## 0.2.0
First release
- UVC/V4L backends
- `Camera` struct for simplification
- `CaptureBackendTrait` to simplify writing backends
