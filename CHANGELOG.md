# Changelog

## Unreleased (0.13.0)

### ⚠ BREAKING CHANGES

* Replaced `CaptureBackendTrait` with four capability-based traits:
  `CameraDevice`, `FrameSource`, `ShutterCapture`, `EventSource`.
* Removed `Camera<F>` and `CallbackCamera<F>`. Replaced by `CameraSession`
  (returning an `OpenedCamera` enum with `Stream`, `Shutter`, `Hybrid`
  variants) and `CameraRunner` (threaded helper).
* Renamed feature `output-threaded` → `runner`; dropped `parking_lot`
  and `arc-swap` deps that only backed the removed `CallbackCamera`.
* `input-opencv` backend temporarily disabled pending migration to the
  new traits (enabling it now triggers a `compile_error!`).
* Backend trait methods `refresh_camera_format`, `resolution`,
  `frame_rate`, `frame_format`, `set_resolution`, `set_frame_rate`,
  `set_frame_format`, and single-control `camera_control(id)` are
  removed. Use `negotiated_format()` / `set_format(CameraFormat)` and
  `controls()` instead.
* See `MIGRATING-0.13.md` for a full step-by-step guide.

### Features

* New `CameraEvent` type and `EventPoll` trait for camera events
  (disconnect, capture error, will-shut-down).
* `CameraRunner` channel-based threaded helper behind the `runner`
  feature, with per-variant loops (stream / shutter / hybrid) and
  configurable queue sizes.
* `nokhwa_backend!` macro for custom-backend crates to declare
  their capability set and obtain the internal `AnyDevice` impl.
* New `testing` feature on `nokhwa-core` providing `MockFrameSource`,
  `MockShutter`, and `MockHybrid` backends for integration tests.
* `CameraSession::open` now dispatches to `V4LCaptureDevice` on Linux
  alongside the AVFoundation and Media Foundation branches. The
  `V4LCaptureDevice<'a>` lifetime parameter was removed; the
  `MmapStream` handle is stored as `'static`. See the struct-level
  docs on `V4LCaptureDevice` for the soundness argument.

### Infrastructure

* `wgpu` helpers (`RawTextureData`, `raw_texture_layout`) moved from
  `nokhwa_core::traits` to a dedicated `nokhwa_core::wgpu` module.
* Workspace version bumped to 0.13.0.
* Pre-commit hook gained a `NOKHWA_SKIP_CLIPPY` escape hatch used
  during the trait-split transition; the workspace clippy is clean at
  release time.

### Documentation

* Added `MIGRATING-0.13.md` covering the 0.12 → 0.13 migration.
* Rewrote top-level `lib.rs` doc comments and README quick-start.
* Migrated all examples (`capture`, `captesting`, `setting`,
  `threaded-capture`) to the new API and added minimal
  `examples/stream_camera.rs` and `examples/runner.rs` at the
  workspace root.

### Additional breaking changes

* `RunnerConfig` has been trimmed to three fields (`poll_interval` —
  renamed from `tick`, `event_tick`, and the new `shutter_timeout`
  defaulting to 5 s, replacing the previously hard-coded 200 ms).
  The vestigial `frames_capacity` / `pictures_capacity` /
  `events_capacity` / `overflow` fields and the `Overflow` enum were
  removed because `std::sync::mpsc::channel` is unbounded. Bounded
  channels with an overflow policy are tracked for 0.14.
* `CameraSession` is now a unit struct; the no-op `CameraSession::new`
  constructor was removed. `CameraSession::open(index, req)` is
  unchanged.

### Diagnostics

* `HybridCamera::from_device` / `CameraRunner::spawn_hybrid` and the
  `CameraRunner` worker thread joins now log event-poller init
  failures and worker panics via `log::warn!` (gated on the `logging`
  feature) instead of swallowing or burying them in an `Option`.

### Internal

* Hidden macro-internal items (`from_device`, `AnyDevice`,
  `HybridBackend`, `CAP_*`) are now `#[doc(hidden)]` throughout.
* Deleted the unused `__nokhwa_cap_bit!` helper macro.
* Replaced stale `tests/device_tests.rs` body with a placeholder
  pending migration to the 0.13 API.

## [0.12.0](https://github.com/DaveDev42/nokhwa/compare/v0.11.0...v0.12.0) (2026-04-12)


### ⚠ BREAKING CHANGES

* type-safe decode API (0.12.0) ([#85](https://github.com/DaveDev42/nokhwa/issues/85))
* restructure error types with structured context, fix UninitializedError typo ([#47](https://github.com/DaveDev42/nokhwa/issues/47))
* remove deprecated API methods (new_with, set_camera_format) ([#44](https://github.com/DaveDev42/nokhwa/issues/44))

### Features

* add frame_texture_raw() for native-format GPU textures ([#50](https://github.com/DaveDev42/nokhwa/issues/50)) ([a83bcd5](https://github.com/DaveDev42/nokhwa/commit/a83bcd5d380d9bb868adf64de88c0600b4c9d50a))
* add frame_timeout() method for bounded frame capture ([#49](https://github.com/DaveDev42/nokhwa/issues/49)) ([daf3475](https://github.com/DaveDev42/nokhwa/commit/daf3475c533941c0d2f6b00b9d39fe5917cda5c3))
* add structured logging behind optional feature flag ([#76](https://github.com/DaveDev42/nokhwa/issues/76)) ([485eebc](https://github.com/DaveDev42/nokhwa/commit/485eebcc2397cceb6f2d2c95e6b9ddaecef85d8b))
* add TimestampKind to Buffer for platform-aware timestamp semantics ([#48](https://github.com/DaveDev42/nokhwa/issues/48)) ([a89b7d8](https://github.com/DaveDev42/nokhwa/commit/a89b7d8e2fbf872d7b20ee171ca0e7da138ada80))
* **core:** port OpenCV Mat conversion to Frame&lt;F&gt; API ([#94](https://github.com/DaveDev42/nokhwa/issues/94)) ([8a5dab9](https://github.com/DaveDev42/nokhwa/commit/8a5dab917fbfb94d236b153a85237ecb80ef26e0))
* type-safe decode API (0.12.0) ([#85](https://github.com/DaveDev42/nokhwa/issues/85)) ([6874fb6](https://github.com/DaveDev42/nokhwa/commit/6874fb6b20cdd282a487977f345653df88a87408))


### Bug Fixes

* address code review — filter logic, stream state, thread lifecycle, API typo ([#39](https://github.com/DaveDev42/nokhwa/issues/39)) ([4f3098e](https://github.com/DaveDev42/nokhwa/commit/4f3098ec5294627def6bddeb879d86e15be208dc))
* **ci:** prevent release-please from bumping to 1.0.0 on breaking changes ([#91](https://github.com/DaveDev42/nokhwa/issues/91)) ([38b879d](https://github.com/DaveDev42/nokhwa/commit/38b879dfbc17a2690bbe952a98d3461e5cac9731))
* **ci:** switch release-please to simple type for workspace compatibility ([#63](https://github.com/DaveDev42/nokhwa/issues/63)) ([4bd8243](https://github.com/DaveDev42/nokhwa/commit/4bd8243eda4ae623c743822ad20f91fdd1dab11a))
* replace unsafe impl Send for Camera with type-level Send bound ([#45](https://github.com/DaveDev42/nokhwa/issues/45)) ([200b6b3](https://github.com/DaveDev42/nokhwa/commit/200b6b355af0ef67b36b7a6885a5ad50432470ae))
* revert workspace version to 0.11.0, will release 0.12.0 when ready ([#87](https://github.com/DaveDev42/nokhwa/issues/87)) ([0cdf3f4](https://github.com/DaveDev42/nokhwa/commit/0cdf3f4119fae9d526f80a74ab6e6042497dc4ec))
* update release-please last-release-sha to current main, cleanup TODO ([#102](https://github.com/DaveDev42/nokhwa/issues/102)) ([252ccf9](https://github.com/DaveDev42/nokhwa/commit/252ccf905244bfd64451404cbffa15d87f078347))


### Performance

* add SIMD-optimized pixel format conversion for YUYV and BGR ([#58](https://github.com/DaveDev42/nokhwa/issues/58)) ([81c4670](https://github.com/DaveDev42/nokhwa/commit/81c4670e525f76f1cf6d3cdd9f3bb71c6c4be3f9))
* add zero-copy Buffer constructors, eliminate redundant frame copies ([#56](https://github.com/DaveDev42/nokhwa/issues/56)) ([d74887b](https://github.com/DaveDev42/nokhwa/commit/d74887be6c549f1d697f903c24f688d2615c6571))
* eliminate double copy in AVFoundation frame capture pipeline ([#52](https://github.com/DaveDev42/nokhwa/issues/52)) ([9ae0609](https://github.com/DaveDev42/nokhwa/commit/9ae0609063cdbd36e227d9af0d4b0803efd0b6a5))
* inline YUV-to-RGB conversion in NV12 decoder ([#67](https://github.com/DaveDev42/nokhwa/issues/67)) ([576e19b](https://github.com/DaveDev42/nokhwa/commit/576e19b09baeabf0fa149240b14fbc8302c74f3a))
* optimize NV12 decoder with pre-computed UV offsets ([#53](https://github.com/DaveDev42/nokhwa/issues/53)) ([6f3929e](https://github.com/DaveDev42/nokhwa/commit/6f3929e1c253f57426cb9724828289605a98f201))
* reduce CallbackCamera lock contention with lock-free last_frame ([#59](https://github.com/DaveDev42/nokhwa/issues/59)) ([b84add4](https://github.com/DaveDev42/nokhwa/commit/b84add42a85d6d9e8351cea4f337d5e3180b48d7))
* SIMD optimizations for all pixel format converters ([#98](https://github.com/DaveDev42/nokhwa/issues/98)) ([17ac2bb](https://github.com/DaveDev42/nokhwa/commit/17ac2bbc4293c3f19c3e3f3abd25a2af9b67949a))
* use unchecked indexing in NV12 scalar decoder hot loops ([#70](https://github.com/DaveDev42/nokhwa/issues/70)) ([031dfe2](https://github.com/DaveDev42/nokhwa/commit/031dfe2b095f9ad1bd0e0e2e5e7f2eb5fd213f73))
* use unchecked indexing in YUYV scalar decoder hot loops ([#73](https://github.com/DaveDev42/nokhwa/issues/73)) ([73576aa](https://github.com/DaveDev42/nokhwa/commit/73576aab676b4aa88a5d231ae869821b9fe12270))
* use unchecked indexing in YUYV/NV12 scalar decoder hot loops ([#69](https://github.com/DaveDev42/nokhwa/issues/69)) ([52cfa3f](https://github.com/DaveDev42/nokhwa/commit/52cfa3feb406bc1beca34bd6b6108021b1eb075d))


### Refactoring

* **core:** split simd.rs into domain-based module directory ([#101](https://github.com/DaveDev42/nokhwa/issues/101)) ([37bbb7a](https://github.com/DaveDev42/nokhwa/commit/37bbb7a501e37fd3392d06effea8ceec615b99e0))
* extract common backend logic to nokhwa-core, normalize query function names ([#80](https://github.com/DaveDev42/nokhwa/issues/80)) ([1adddeb](https://github.com/DaveDev42/nokhwa/commit/1adddeb767e83966ec03d8038910d0aa8069bbfa))
* improve Camera API ergonomics ([#77](https://github.com/DaveDev42/nokhwa/issues/77)) ([2b9a4d7](https://github.com/DaveDev42/nokhwa/commit/2b9a4d706e7992598f4a2c769bd63719945804b2))
* **macos:** reduce unsafe surface area with safe wrapper methods ([#78](https://github.com/DaveDev42/nokhwa/issues/78)) ([5b834f7](https://github.com/DaveDev42/nokhwa/commit/5b834f79634787382eb6cc4adf0d19e0eda048e2))
* replace backend dispatch macros with explicit factory functions ([#43](https://github.com/DaveDev42/nokhwa/issues/43)) ([f9fe9f6](https://github.com/DaveDev42/nokhwa/commit/f9fe9f69e8d28879f440a9b48c077834c0a180b8))
* restructure error types with structured context, fix UninitializedError typo ([#47](https://github.com/DaveDev42/nokhwa/issues/47)) ([3fd2fe0](https://github.com/DaveDev42/nokhwa/commit/3fd2fe0811aad9e5aed7f9d0d25c466c82791c3e))
* restructure OpenDeviceError with named fields ([#66](https://github.com/DaveDev42/nokhwa/issues/66)) ([21c1471](https://github.com/DaveDev42/nokhwa/commit/21c1471d9b808e559b397688d8229107836ac8a1))
* simplify recently changed code — reduce duplication across Frame/Camera/SIMD ([#105](https://github.com/DaveDev42/nokhwa/issues/105)) ([935ffc2](https://github.com/DaveDev42/nokhwa/commit/935ffc248976d20c869f3d0005c83ced14dc6cc8))


### Infrastructure

* add Claude Code local files and planning artifacts to .gitignore ([#89](https://github.com/DaveDev42/nokhwa/issues/89)) ([6c7f19e](https://github.com/DaveDev42/nokhwa/commit/6c7f19e89fd5ec1578b870a04aacf9aff6e182c0))
* **ci:** correct release-please baseline to actual v0.11.0 commit ([#111](https://github.com/DaveDev42/nokhwa/issues/111)) ([66ae79c](https://github.com/DaveDev42/nokhwa/commit/66ae79c51904a1517d898c7fd2f63bc108b6a18f))
* **ci:** force release-please to 0.12.0 via release-as override ([#113](https://github.com/DaveDev42/nokhwa/issues/113)) ([4bddb78](https://github.com/DaveDev42/nokhwa/commit/4bddb78e6c982847b7a05b6949031d9225d5a906))
* **ci:** set release-please baseline to v0.11.0 commit ([#109](https://github.com/DaveDev42/nokhwa/issues/109)) ([f5c9a4f](https://github.com/DaveDev42/nokhwa/commit/f5c9a4f5e81fc70c5aaee34eab33004a325897a2))
* **examples:** migrate to 0.12.0 Frame&lt;F&gt; / Camera&lt;F&gt; API ([#106](https://github.com/DaveDev42/nokhwa/issues/106)) ([d9bb67f](https://github.com/DaveDev42/nokhwa/commit/d9bb67f2c97037cb5ff522507ee0400b5e01136f))
* gitignore Claude Code runtime files ([#82](https://github.com/DaveDev42/nokhwa/issues/82)) ([7108742](https://github.com/DaveDev42/nokhwa/commit/7108742987709041431b7e27aea3c8b2ee0e8391))
* improve feature flag discoverability with compile-time checks and documentation ([#74](https://github.com/DaveDev42/nokhwa/issues/74)) ([5af7c79](https://github.com/DaveDev42/nokhwa/commit/5af7c798dcb0a3af671a01d3aa6aa3b81ba1e29c))
* **main:** release 0.11.1 ([#64](https://github.com/DaveDev42/nokhwa/issues/64)) ([c6b0053](https://github.com/DaveDev42/nokhwa/commit/c6b0053ac6c74620280fc417c671069502bcb4b5))
* make image crate dependency optional, gated behind decoding feature ([#81](https://github.com/DaveDev42/nokhwa/issues/81)) ([addbe44](https://github.com/DaveDev42/nokhwa/commit/addbe4495f4c7f0fa3cae8a3143a2c3b24bc4a3c))
* remove deprecated API methods (new_with, set_camera_format) ([#44](https://github.com/DaveDev42/nokhwa/issues/44)) ([5c4a7cc](https://github.com/DaveDev42/nokhwa/commit/5c4a7cc1fa39aefb96554e6c425a2b5534622578))
* replace once_cell with std::sync::LazyLock ([#75](https://github.com/DaveDev42/nokhwa/issues/75)) ([f436290](https://github.com/DaveDev42/nokhwa/commit/f43629095b582950531fb154290a6ad69f992da3))
* set up release-please for automated patch versioning ([#62](https://github.com/DaveDev42/nokhwa/issues/62)) ([3f606f1](https://github.com/DaveDev42/nokhwa/commit/3f606f1abcd674ecfefafaefe595fb1ab83a3d38))


### Documentation

* add 0.13.0 roadmap — separate streaming vs still-image capture ([#99](https://github.com/DaveDev42/nokhwa/issues/99)) ([5f2cd7c](https://github.com/DaveDev42/nokhwa/commit/5f2cd7c944a174ed05ce8c824c0d3c57f2aea3f0))
* add comprehensive SIMD performance items to TODO.md ([#95](https://github.com/DaveDev42/nokhwa/issues/95)) ([4633cf2](https://github.com/DaveDev42/nokhwa/commit/4633cf269e869e92894aca6fe22fc66edfa07d7a))
* add new improvement items to TODO.md, add gw TODO rule to CLAUDE.md ([#72](https://github.com/DaveDev42/nokhwa/issues/72)) ([e5c1a7f](https://github.com/DaveDev42/nokhwa/commit/e5c1a7f65f3a16047be931600aa8eae26d5222c8))
* add performance improvement items to TODO.md ([#51](https://github.com/DaveDev42/nokhwa/issues/51)) ([1516922](https://github.com/DaveDev42/nokhwa/commit/15169220b97882aa140cabe2aa0ec6458bc2a6f8))
* add simd.rs module split task, mark SIMD items completed ([#100](https://github.com/DaveDev42/nokhwa/issues/100)) ([bd66460](https://github.com/DaveDev42/nokhwa/commit/bd66460b63933020c302c0f996342f3531ce2ae7))
* add simplify review, docs update, examples update, benchmarks to TODO.md ([#104](https://github.com/DaveDev42/nokhwa/issues/104)) ([10a5509](https://github.com/DaveDev42/nokhwa/commit/10a55090fca3a601c144911283b239bb05080e49))
* clean up TODO.md — remove all completed items from recent PRs ([#83](https://github.com/DaveDev42/nokhwa/issues/83)) ([11427f3](https://github.com/DaveDev42/nokhwa/commit/11427f32545a3cfa8332b37d49de9509e9938dd6))
* **core:** replace ignore doc-tests with compilable examples ([#40](https://github.com/DaveDev42/nokhwa/issues/40)) ([187b182](https://github.com/DaveDev42/nokhwa/commit/187b182a102df4d3138ddf64d6784c515ae6a892))
* fix stale YUYV comments, update TODO.md for completed performance items ([#60](https://github.com/DaveDev42/nokhwa/issues/60)) ([e6033e6](https://github.com/DaveDev42/nokhwa/commit/e6033e6e53b17084b6c5c5354099608d6f6f02f4))
* improve Camera, lib.rs, RequestedFormat, CaptureBackendTrait, and examples documentation ([#79](https://github.com/DaveDev42/nokhwa/issues/79)) ([6f5bf2d](https://github.com/DaveDev42/nokhwa/commit/6f5bf2d6249e1369575435ff9c4dda291cd05a2a))
* mark CallbackCamera Drop panic as already fixed in TODO.md ([#42](https://github.com/DaveDev42/nokhwa/issues/42)) ([c773b12](https://github.com/DaveDev42/nokhwa/commit/c773b12b0ff95e051c47d908c2f9294c4d27d983))
* remove completed items from TODO.md ([#96](https://github.com/DaveDev42/nokhwa/issues/96)) ([5aae15f](https://github.com/DaveDev42/nokhwa/commit/5aae15f307004fe1483d3ed3a75d591114e3c4a6))
* remove completed items from TODO.md for readability ([#61](https://github.com/DaveDev42/nokhwa/issues/61)) ([afdfb10](https://github.com/DaveDev42/nokhwa/commit/afdfb103beb07b981fa9ee6246d191756657aaa1))
* remove completed OpenDeviceError and NV12 inline items from TODO.md ([#68](https://github.com/DaveDev42/nokhwa/issues/68)) ([fe8e1d4](https://github.com/DaveDev42/nokhwa/commit/fe8e1d4ee33d681dda3a4f00af1e335a09d40a66))
* separate MJPEG unit tests from integration tests in TODO.md ([#92](https://github.com/DaveDev42/nokhwa/issues/92)) ([fd7052d](https://github.com/DaveDev42/nokhwa/commit/fd7052d3f6b35c5c1452cb28d85b84dcbf5278d2))
* update CHANGELOG and TODO for NV12 decoder optimization ([#53](https://github.com/DaveDev42/nokhwa/issues/53)) ([#54](https://github.com/DaveDev42/nokhwa/issues/54)) ([ee88024](https://github.com/DaveDev42/nokhwa/commit/ee88024f2f950846bbf571c3e70c6642adf9ee97))
* update README, lib.rs, and add migration guide for 0.12.0 API ([#108](https://github.com/DaveDev42/nokhwa/issues/108)) ([3ebfe03](https://github.com/DaveDev42/nokhwa/commit/3ebfe03c6b7d16b2968b16d2c911de842d312263))
* update TODO.md — reflect NV12 unchecked indexing done in [#70](https://github.com/DaveDev42/nokhwa/issues/70) ([#71](https://github.com/DaveDev42/nokhwa/issues/71)) ([f3c5a2e](https://github.com/DaveDev42/nokhwa/commit/f3c5a2e32f066d9f4d80d692011e6f8d91ce4bc0))
* update TODO.md — remove completed items, clean up stale entries ([#65](https://github.com/DaveDev42/nokhwa/issues/65)) ([3c6f5bb](https://github.com/DaveDev42/nokhwa/commit/3c6f5bb485c0491d6d44148754e76135c0c4fb2f))
* update TODO.md with structural improvement items from project review ([#41](https://github.com/DaveDev42/nokhwa/issues/41)) ([c7333eb](https://github.com/DaveDev42/nokhwa/commit/c7333eb9f542b6786606ee5d3dacae962548ef43))


### Testing

* add format conversion, control round-trip, and robustness tests ([#46](https://github.com/DaveDev42/nokhwa/issues/46)) ([1afaa2b](https://github.com/DaveDev42/nokhwa/commit/1afaa2b7bcbbdd9ecd169d313bfb90d20780676a))
* **core:** add MJPEG positive correctness and robustness unit tests ([#93](https://github.com/DaveDev42/nokhwa/issues/93)) ([8e87327](https://github.com/DaveDev42/nokhwa/commit/8e87327cb311a2dd1eeca3524dbf1a21eaea94b6))

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

### Refactoring

* **SIMD module split**: `nokhwa-core/src/simd.rs` (2,150+ lines) split into `simd/` module directory organized by conversion domain (`bgr_to_rgb`, `yuyv_to_rgb`, `nv12_to_rgb`, `rgb_to_rgba`, `yuyv_extract_luma`, `rgb_to_luma`). Pure refactor — no behavior changes.

### Bug Fixes

* **wgpu**: Fixed `frame_texture()` writing to `mip_level: 1` instead of `mip_level: 0` (base level).

### Docs

* **README**: Rewrote Quick Start for the `Camera::open::<F>()` / `frame_typed()` / `into_rgb().materialize()` flow and added a compile-fail demo for `Frame<Gray>`.
* **nokhwa-core**: Added module-level overview covering `Buffer`, `Frame<F>`, `CaptureFormat` markers, and the `IntoRgb`/`IntoRgba`/`IntoLuma` lazy-conversion traits.
* **nokhwa**: Expanded top-level module docs with Key Types for `Camera<F>`, `CallbackCamera<F>`, and `Frame<F>`.
* **MIGRATION-0.12.md**: New migration guide from 0.11.x with API map, before/after examples, removed items, and a format picker table.

### Cleanup

* **examples**: Updated `examples/` to the 0.12.0 typed API (`Camera::open::<F>`, `Camera<F>::frame_typed`, `Frame<F>` + `IntoRgb`/`IntoRgba`); added READMEs for `captesting`, `decoder_test`, and `threaded-capture`; each example now opts out of the root workspace so it builds standalone.

### Infrastructure

* **Benchmarks**: Added criterion benchmarks for pixel format conversions (BGR→RGB, YUYV→RGB/RGBA, NV12→RGB/RGBA, RGB→Luma, YUYV Y-extraction) at 640×480, 1920×1080, and 3840×2160. Compares SIMD vs scalar with per-benchmark correctness checks. Gated behind the internal `bench` Cargo feature in `nokhwa-core` (not part of the stable API).

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
