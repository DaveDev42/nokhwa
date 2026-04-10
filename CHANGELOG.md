# 0.11.0 (unreleased, fork: DaveDev42/nokhwa)

## Breaking
- Removed deprecated `Camera::new_with()` (use `Camera::new()` or `Camera::with_backend()` instead)
- Removed deprecated `Camera::set_camera_format()` and `CallbackCamera::set_camera_format()` (use `set_camera_request()` instead)
- Renamed bindings crates: `nokhwa-bindings-macos` → `nokhwa-bindings-macos-avfoundation`, `nokhwa-bindings-windows` → `nokhwa-bindings-windows-msmf`, `nokhwa-bindings-linux` → `nokhwa-bindings-linux-v4l`
- Unified all workspace crate versions to `0.11.0` (workspace version inheritance)
- Moved `CaptureBackendTrait` impl from root crate wrappers into bindings crates (consistent with Linux pattern)
- Replaced `flume` with `std::sync::mpsc` (API compatible but different error types)

## Performance
- CallbackCamera threading overhaul: eliminated simultaneous multi-lock, fixed memory ordering (SeqCst → Release/Acquire), added thread join in Drop
- Replaced `to_vec()` + sort allocations with zero-allocation `max_by_key` iterators in `RequestedFormat::fulfill()`
- Deduplicated Windows Media Foundation format enumeration (~80 lines removed)
- Removed unnecessary `Vec::default()` allocations in CallbackCamera

## Refactoring
- **macOS: migrated from `objc`/`cocoa-foundation` to `objc2`/`block2`** — eliminated all 186 deprecation warnings, reduced dependencies from 6 to 3
- Split macOS bindings monolith (2,422 lines) into 6 focused modules (ffi, util, types, callback, device, session)
- Fixed UB: `from_raw_parts_mut` → `from_raw_parts` in CVPixelBuffer callback

## Features
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

## Cleanup
- Replaced `flume` crate with `std::sync::mpsc` to reduce external dependencies (all channel usages migrated in library and examples)
- Replaced `core-media-sys` / `core-video-sys` crate dependencies with direct FFI declarations in `ffi.rs`, eliminating legacy `objc 0.2` and `metal 0.18` transitive dependencies
- Removed unused dependencies: `usb_enumeration`, `regex`, `cocoa-foundation`, `core-foundation`, `once_cell`
- Removed dead code: empty `VirtualBackendTrait`, commented-out module declarations, obsolete code blocks
- Removed obsolete `make-npm.sh` (JS bindings removed in 0.10.0)

# 0.10.0
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

# 0.9.0
- Fixed Camera Controls for V4L2
- Disabled UVC Backend.
- Added polling and last frame to `ThreadedCamera`
- Updated the `CameraControl` related Camera APIs

# 0.8.0
- Media Foundation Access Violation fix (#13)

# 0.7.0
- Bumped some dependencies.

# 0.5.0
 - Fixed `msmf`
 - Relicensed to Apache-2.0

# 0.4.0
- Added AVFoundation, MSMF, WASM
- `.get_info()` returns a `&CameraInfo`
- Added Threaded Camera
- Added JSCamera
- Changed `new` to use `CaptureAPIBackend::Auto` by default. Old functionally still possible with `with_backend()`
- Added `query()`, which uses `CaptureAPIBackend::Auto` by default.
- Fixed/Added examples

# 0.3.2
- Bumped `ouroboros` to avoid potential UB
- [INTERNAL] Removed `Box<T>` from many internal struct fields of `UVCCaptureDevice`

# 0.3.1
- Added feature hacks to prevent gstreamer/opencv docs.rs build failure

# 0.3.0
- Added `query_devices()` to query available devices on system
- Added `GStreamer` and `OpenCV` backends
- Added `NetworkCamera`
- Added WGPU Texture and raw buffer write support
- Added `capture` example
- Removed `get_` from all APIs. 
- General documentation fixes
- General bugfixes/performance enhancements


# 0.2.0
First release
- UVC/V4L backends
- `Camera` struct for simplification
- `CaptureBackendTrait` to simplify writing backends
