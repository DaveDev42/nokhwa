# TODO

Working list. Short lines only — rationale + implementation notes live
in `CHANGELOG.md`, PR descriptions, and commit messages.

## Open

> Device-testing strategy (see CLAUDE.md → "Testing Strategy"): Linux
> real-camera coverage runs in CI via `v4l2loopback`; **macOS and Windows
> device tests are run on the maintainer's own hardware**, not on
> GitHub-hosted runners (no usable virtual camera exists there). Hosted
> CI still covers logic/stub/build tests on all three OSes.

### Runtime verification pending (compile-verified only)

- [ ] **MSMF `set_control` forces manual mode when writing an explicit value (fix/msmf-set-control-force-manual)** —
  `set_control` read the device's current auto/manual flag via `self.control(control)?` and forwarded
  that same flag to `IAMVideoProcAmp::Set`/`IAMCameraControl::Set`. When the device was in Auto mode,
  writing an explicit value sent `CameraControl_Flags_Auto`, causing the driver to silently ignore the
  value — Auto→Manual transitions were impossible via `set_control`. Now always passes
  `CameraControl_Flags_Manual` (writing a specific value inherently means manual).
  Dropped the `self.control(control)?` call (was used only to derive the flag) and removed
  the now-unused `CameraControl_Flags_Auto` import. Compile-checked in CI. Verify on Windows hardware
  that `set_control(Brightness, N)` on an auto-mode camera actually applies N:
  `cargo test --features device-test,input-msmf,runner`

- [ ] **MSMF `raw_bytes` gratuitous `MFCreateSample` removed (perf/msmf-drop-gratuitous-mfcreatesample)** —
  Pre-allocating an `IMFSample` before calling `ReadSample` was wasted: `ReadSample` COM-releases
  the pre-created object and replaces it with the captured sample. Changed initializer to `None`;
  `MFCreateSample` import removed. Compile-checked in CI. Verify on Windows hardware that frame
  capture still works correctly (no regression):
  `cargo test --features device-test,input-msmf,runner`

- [ ] **MSMF `raw_bytes` Lock/Unlock + stream-end guard (fix/msmf-raw-bytes-unlock-and-stream-end-guard)** —
  `IMFMediaBuffer::Lock` was never paired with `Unlock` on any return path (success or the two
  early-error paths for null pointer / zero length). The read loop also spun forever when
  `ReadSample` returned `Ok` but set `MF_SOURCE_READERF_ERROR` or `MF_SOURCE_READERF_ENDOFSTREAM`
  in `stream_flags` (camera unplugged mid-stream). Both fixed together: `Unlock` called before
  every return after a successful `Lock`; the loop now checks the error/end-of-stream bits before
  the `is_some()` break and returns `Err(ReadFrameError { "stream ended or errored" })`. Compile-
  checked in CI (Build/Clippy windows). Verify on Windows hardware that frames keep flowing across
  many reads and that unplugging mid-stream surfaces an error instead of hanging:
  `cargo test --features device-test,input-msmf,runner` +
  `cargo run --example hotplug_probe`
- [ ] **MSMF hotplug channel-closed sentinel (fix/msmf-hotplug-doc-and-channel-sentinel, F7)** —
  `wnd_proc` now calls `PostQuitMessage(0)` when `reconcile_and_emit` returns `false` (channel
  closed). Verify on Windows hardware: hotplug still emits `Connected` → `Disconnected` during
  normal operation and that after dropping the poller, further `WM_DEVICECHANGE` events no longer
  trigger wasted `take_snapshot()` MF enumeration. Run `cargo run --example hotplug_probe` on Windows.
- [ ] **GStreamer extra-control truncation guard + URI per-sample format (fix/gstreamer-extra-control-truncation-and-uri-per-sample-format)** —
  `build_extra_controls` now returns `Result<Option<…>, NokhwaError>` and propagates a
  `SetPropertyError` if any pending `i64` value exceeds `i32` range (V4L2 CIDs are `__s32`).
  All 3 callers in `lib.rs` updated. `pull_frame` in `uri.rs` now re-derives the format from
  each sample's caps via `sample_format(&sample)` instead of the cached `self.format`, so
  adaptive/renegotiating RTSP/HLS streams report correct dimensions per frame.
  Compile-checked (`cargo check -p nokhwa-bindings-gstreamer`). Verify on Linux with a
  real/URL GStreamer source: `cargo test --features device-test,input-gstreamer` and confirm
  (1) setting an out-of-range extra-control returns an error, and (2) per-sample format
  matches actual frame bytes on a renegotiating stream.

- [ ] **AVF re-open guard + `frame_raw` drain (fix/avf-guard-reopen-and-drain-frame-raw)** —
  `open()` now returns `Ok(())` immediately if `is_open()` is true, preventing a double-open
  that would leak the previous `AVCaptureSession` and its delegate (old session kept running,
  double-feeding the channel). `frame_raw()` now drains stale queued frames with
  `try_iter().for_each(drop)` after `recv()`, matching `frame()`, so callers get the freshest
  frame rather than a growing backlog. Compile/clippy-verified (macOS). Verify on hardware:
  `cargo test --features device-test,input-avfoundation,runner` — confirm that calling `open()`
  on an already-open camera is a no-op, and that `frame_raw()` returns the same freshness as
  `frame()` (no stale-backlog drift over time).
- [ ] **AVF ExposureDuration/ISO writability gated on active Custom mode (fix/avf-lock-leak-and-exposure-control-wiring)** —
  `ExposureDuration` (`KnownCameraControl::Gamma`) and `ExposureISO` (`KnownCameraControl::Brightness`)
  now gate their `ReadOnly` flag and `active` bool on `exposure_is_custom` (device is *currently* in
  Custom exposure mode) instead of `exposure_custom` (Custom mode is merely *supported*). AVFoundation
  only allows `setExposureModeCustomWithDuration:ISO:` when actively in Custom mode; the old code made
  both controls appear writable when Custom was supported but the device was in a different mode.
  Compile-verified on macOS. Verify on hardware: switch to Custom exposure mode and confirm
  `ExposureDuration` + `ExposureISO` surface as writable+active; switch back and confirm ReadOnly+inactive.
  `cargo test --features device-test,input-avfoundation,runner --test device_tests -- --test-threads=1`

- [ ] **MSMF control() helper extraction (refactor/msmf-control-extract-helpers)** —
  Mechanical extraction of `GetRange`+`Get` boilerplate from `control()` into
  `query_proc_amp` / `query_camera_control` helpers. Behaviour-preserving by
  construction; error message text unchanged. Verify control read-back on Windows
  hardware: `cargo test --features device-test,input-msmf,runner` — confirm that
  Brightness / Contrast / Exposure / Focus / Zoom and other `KnownCameraControl`
  values round-trip correctly through `control()`.

- [ ] **MSMF CAMERA_REFCNT atomic RMW fix (fix/msmf-camera-refcnt-atomic-rmw)** —
  `fetch_add`/`fetch_sub` 전환. Windows에서 멀티스레드 동시 open/close가
  refcount 정확성을 유지하는지 확인. 단일 스레드 동작은 변경 없음.
  `cargo test --features device-test,input-msmf,runner` on Windows hardware.

- [ ] **MSMF init completion-ordering fix (fix/msmf-init-completion-ordering)** —
  `INITIALIZED` was an `AtomicBool` set via `compare_exchange`. The CAS
  guaranteed a single `MFStartup`, but a lost-race caller observed `true` and
  proceeded to call MF APIs while the CAS winner was *still inside* `MFStartup`
  — UB, since MF must be fully started before use and `MFStartup` is not
  re-entrant. Switched the flag to `Mutex<bool>` held across `CoInitializeEx +
  MFStartup` (and mirrored across `MFShutdown + CoUninitialize`), so lost-race
  callers block until init *completes*. Retry-on-failure preserved (flag stays
  `false` on error); poisoned lock surfaces as `InitializeError`/`ShutdownError`.
  Compile-checked in CI (Build/Clippy windows); the real module is
  `cfg(target_os = "windows")` so it cannot be checked on the macOS dev box.
  Verify on Windows hardware that concurrent `initialize_mf` from multiple
  threads still starts MF exactly once and single-threaded open/close is
  unaffected: `cargo test --features device-test,input-msmf,runner`.

- [ ] **GStreamer restart-state fixes + controls() error variant (fix/gstreamer-pipeline-restart-state)** —
  Three bug fixes in `nokhwa-bindings-gstreamer/src/lib.rs`, compile-verified only on macOS
  (no GStreamer dev libs). (G1) `controls()` previously returned `ReadFrameError` when the
  pipeline was closed; changed to `GetPropertyError` (semantically correct: property
  introspection, not frame capture). (G2) `set_format()` previously mutated
  `local.negotiated` before the restart succeeded, leaving the device with an incorrect
  format on failure; now builds the replacement pipeline first and only commits the new
  format + pipeline on success. (G3) `set_control(V4l2Cid)` previously set
  `self.pipeline = None` before the restart, leaving the device closed on failure; now
  builds the replacement pipeline before overwriting `self.pipeline` so the old pipeline
  keeps running on failure. Verify on a Linux box with `v4l2loopback` or a real webcam:
  `cargo test -p nokhwa-bindings-gstreamer --features input-gstreamer` and
  `cargo test --features device-test,input-gstreamer` — confirm that `controls()` errors
  cleanly on a closed pipeline, that `set_format()` with a bad format leaves the device
  streaming the old format, and that a failed `set_control(V4l2Cid)` leaves the device
  still streaming.

- [ ] **GStreamer _touch_unsupported cleanup (refactor/gst-remove-touch-unsupported-workaround)** —
  Dropped the dead-code lint workaround function and its companion
  `unsupported` import. macOS does not have GStreamer dev libs locally, so
  verify Linux/Windows backend builds pass clippy with `-D warnings` in CI
  and that controls path still functions on Linux hardware.

- [ ] **GStreamer cleanup bundle (refactor/gst-snapshot-and-init-helpers)** —
  `ensure_gst_init()` + `snapshot_video_devices()` consolidation has only
  `docs-only` + CI cross-platform compile coverage. Verify on a Linux box
  with libgstreamer1.0-dev installed: `cargo test -p nokhwa-bindings-gstreamer
  --features input-gstreamer`.

- [ ] **V4L FrameInterval helper refactor (refactor/v4l-frame-interval-helper)** —
  `expand_frame_interval()` consolidation + Stepwise inclusive-bound fix
  has only logic/stub CI coverage. Verify on Linux hardware with a webcam
  that reports Stepwise intervals (`cargo test --features device-test,input-v4l,runner`).

- [ ] **V4L controls() message + INACTIVE dedup (refactor/v4l-controls-messages)** —
  Replaced the "what is this?????? todo: support ig" + "Failed to Fufill"
  strings with descriptive errors, stripped the legacy uwu comment, and
  folded the duplicate `INACTIVE → Disabled` mapping into a single
  `DISABLED | INACTIVE` check. Behaviour-preserving: the `active` field on
  the `CameraControl` still keys off `Flags::INACTIVE` only. Verify on
  Linux hardware that `controls()` still surfaces the same flag set on a
  webcam exercising AUTO_GAIN / GAIN gating.

- [ ] **MSMF device-enumeration CoTaskMem leak fixes (fix/msmf-free-cotaskmem-allocations-from-device-enumeration)** —
  Two CoTaskMem leaks fixed in the device-enumeration path. (1) `MFEnumDeviceSources` allocates a
  heap array of `Option<IMFActivate>` pointers; the caller must `CoTaskMemFree` it after use. The
  old code iterated via `from_raw_parts` + `clone()` but never freed the array, leaking it on every
  `query()` / device refresh. Fixed: elements are moved out via `ptr::read` (so each owned
  `IMFActivate` gets Released via Drop exactly once), then `CoTaskMemFree` is called on the array
  pointer. (2) `GetAllocatedString` allocates two PWSTR buffers (friendly name + symbolic link) that
  the caller must `CoTaskMemFree`. They were never freed — leaking on every successfully enumerated
  device, and on error paths too. Fixed: a `free_pwstr` helper null-checks then `CoTaskMemFree`s;
  each PWSTR is converted to a Rust `String` and freed immediately, so later `?` returns cannot
  skip a free. Compile-checked on macOS (no LINK error = compile-clean). Verify on Windows
  hardware that repeated `query()` calls + `cargo run --example hotplug_probe` show no handle or
  memory growth:
  `cargo test --features device-test,input-msmf,runner` +
  `cargo run --example hotplug_probe --features input-msmf`

- [ ] **MSMF COM service helper refactor (refactor/msmf-com-service-helper)** —
  `get_camera_control_services()` consolidation has only the `Build (windows)`
  compile check. Verify control read/write on physical Windows hardware
  (`cargo test --features device-test,input-msmf,runner` on a Win box with a webcam).
- [ ] **MSMF FIRST_VIDEO_STREAM const unification (refactor/msmf-unify-first-video-stream-const)** —
  로컬 상수를 windows-rs import로 통일. 값 동일하지만 Windows에서 enum/format 열거가
  변함없이 동작하는지 확인: `cargo test --features device-test,input-msmf,runner` on
  a Windows box with a webcam attached.

- [ ] **`HybridCamera::frame_raw` runtime path (#445)** — added for parity with
  `StreamCamera` but compile-verified only; FaceTime HD opens as `StreamCamera`
  (AVFoundation advertises no `CAP_SHUTTER`), so no `HybridCamera`-capable backend
  exercised it. Verify on a backend advertising both `CAP_STREAM` + `CAP_SHUTTER`
  (GStreamer/MSMF hybrid path) that `frame_raw()` returns the same bytes as
  `frame().buffer()`.

- [ ] **`HybridCamera::compatible_formats` / `compatible_fourcc` runtime path** —
  Added for parity with `StreamCamera` (same gap as `frame_raw` #445) but
  compile-verified only; FaceTime HD opens as `StreamCamera` (no `CAP_SHUTTER`).
  Verify on a backend advertising both `CAP_FRAME` + `CAP_SHUTTER` that these
  methods return the same result as calling the methods directly on the inner
  `FrameSource` backend.

### Infrastructure / CI

- [ ] **Provision `RELEASE_PLEASE_TOKEN` repo secret.** The
  `release-please.yml` workflow prefers a maintainer-supplied PAT
  (fine-grained: `Contents: r/w` + `Pull requests: r/w` on this repo;
  or classic with `repo`) and falls back to `GITHUB_TOKEN` when unset.
  With the PAT, release-please's push to the release-PR branch
  triggers `pull_request` CI normally; without it, the release PR
  stays BLOCKED on required status checks until someone closes-and-
  reopens it (v0.14.6, v0.14.7, v0.14.8, v0.14.10 all hit this).
  Note: the `workflow_dispatch`-based no-PAT workaround attempted in
  v0.14.10 (PRs #398/#399) was reverted — `workflow_dispatch`-sourced
  check runs do not count toward GitHub's PR required-status-checks
  rollup, so the BLOCKED state persisted. PAT is the proven path.
  See CLAUDE.md → "Commit & Release Convention".

### Backlog

- [ ] **V4L `controls()` drops WRITE_ONLY / INACTIVE controls.** In
  `nokhwa-bindings-linux-v4l/src/lib.rs::controls()`, each descriptor's current
  value is read via `device.control(desc.id)?` (line ~510) before the control is
  emitted. For `WRITE_ONLY` controls `VIDIOC_G_EXT_CTRLS` returns `EACCES`, and
  `INACTIVE` controls (e.g. `GAIN` while `AUTO_GAIN` is on) can also fail the read;
  the `?` errors inside the map closure and `.filter_map(Result::ok)` silently drops
  them. So those controls never appear in `controls()` output even though their flag
  metadata (`WriteOnly` / `Disabled` / `active=false`) is already computed. Proper fix:
  for descriptors flagged `WRITE_ONLY` / `INACTIVE`, skip the value read and emit the
  control with a sentinel value (e.g. `desc.default`) so it's still enumerated with
  correct flags. Deferred from the `set_control` verify fix because it changes
  enumeration output and needs hardware verification on a UVC camera with auto-gated
  controls (`cargo test --features device-test,input-v4l,runner`). NOTE: the
  `set_control` read-back-verify path no longer depends on this (it now inspects the
  descriptor flags directly), so this is purely an enumeration-completeness gap.

- [ ] **`CameraRunner` stream-only frame-error observability.** In
  `src/runner.rs::spawn_stream`, a persistent `cam.frame()` error only drives
  exponential backoff + (optional) logging; the error is never surfaced to the
  consumer, and stream-only runners have no events channel (`events: None`). A
  slow/unplugged camera looks identical to "no frames yet". Fixing it properly
  means adding an error/event channel to the stream-only runner surface — a
  config + public-API change, not a contained bug fix. Deferred until there's a
  consumer that needs programmatic error detection (today: rely on frame
  starvation + `logging` feature).

- [ ] **WASM / browser backend.** Blocked on five design decisions, no
  active consumer:
  - interop library (`tsify` vs `serde-wasm-bindgen` vs hand-rolled)
  - `ApiBackend::Custom(String)` representation in JS
  - frame transport (`Uint8Array` / `OffscreenCanvas` / `ImageBitmap`)
  - `NokhwaError` → JS Error translation
  - browser capture API (`getUserMedia` + `MediaStreamTrackProcessor` vs
    `ImageCapture`)

## Closed — not returning

- **UVC backend** (removed 2026-04-22, before first release) — rationale
  in `CHANGELOG.md`. Windows `usbvideo.sys` owns the interface;
  Linux/macOS have better native paths; no `rusb`/`nusb` public iso
  API. Future niche needs get purpose-built backends, not a generic
  libusb-UVC resurrection.
- **OpenCV capture backend** (removed 2026-04-22 / 0.14.3) — GStreamer
  covers local capture + controls + URL sources first-class now.
  `opencv-mat` (`nokhwa-core` feature for `cv::Mat` interop) is
  unchanged; enable directly if you want the conversion helpers.
- **MSMF device tests on GH-hosted `windows-latest`** (decided
  2026-05-12, not pursuing) — no fakeable MF device source on a hosted
  runner: OBS virtualcam is a DirectShow filter invisible to
  `MFEnumDeviceSources`; the Win11 Camera Extension sample needs a
  code-signing cert GH Actions can't supply; a hand-rolled Rust MF
  source is ~500 LOC `unsafe` FFI of unverified feasibility. Per the
  testing strategy (CLAUDE.md), MSMF device tests run on the
  maintainer's own Windows hardware instead. `msmf-obs-virtualcam.yml`
  deleted (was a `workflow_dispatch`-only diagnostic harness).
- **OBS virtualcam MSMF CI spike** (abandoned 2026-04-21) — OBS
  virtualcam is a DirectShow filter; `MFEnumDeviceSources` and
  DirectShow are disjoint enumeration namespaces. No amount of OBS
  configuration bridges that. (Workflow file removed 2026-05-12 — see
  above.)
- **macOS GH-hosted virtual camera** — not feasible. Modern vcams need
  system extensions codesigned + notarized + installed from
  `/Applications`; GH-hosted macOS runners have no Apple Developer
  credentials. AVFoundation device-test coverage runs on the
  maintainer's own Mac (the self-hosted `macos-camera` runner is one).
- **Network/IP camera backend** — superseded by GStreamer session 5's
  URL path. `CameraIndex::String("rtsp://…")` / `https://…` / `file://…`
  dispatches through `uridecodebin`.

## Shipped recently (for context)

- **V4L `set_control` skips read-back verify for WRITE_ONLY / VOLATILE controls (fix/v4l-set-control-verify-write-only-volatile)** —
  `set_control` in `nokhwa-bindings-linux-v4l/src/lib.rs` performed the `VIDIOC_S_EXT_CTRLS`
  write, then re-read the value via `self.camera_control(id)` and returned
  `SetPropertyError("Rejected")` if the read-back differed. That verify step broke two
  legitimate control classes. (1) **WRITE_ONLY** controls (common on UVC cameras for
  relative pan/tilt/zoom/focus/iris) reject `VIDIOC_G_EXT_CTRLS` with `EACCES`, so
  `controls()` — which reads every control's value up front — silently dropped them, and
  `camera_control(id)` then returned "not found", making a *successful* write look failed.
  (2) **VOLATILE** controls (e.g. `EXPOSURE` under auto-exposure) report a hardware-updated
  value that legitimately differs from what was just written, producing a false "Rejected".
  Fix: after the write succeeds, look up the target control's descriptor flags directly via
  `device.query_controls()` (not the lossy `controls()` round-trip) and skip the read-back
  verify when `WRITE_ONLY | VOLATILE` is set — the kernel already errors on the write itself
  if it was actually rejected. The mutex guard is explicitly dropped before
  `camera_control(id)` re-locks (std Mutex is non-reentrant). Real path is
  `#[cfg(target_os = "linux")]`-gated; compile-verified via CI's Linux build + V4L loopback
  job. Needs hardware verification on a UVC camera exposing WRITE_ONLY (relative PTZ) and
  VOLATILE (auto-exposure) controls: `cargo test --features device-test,input-v4l,runner`.
  The related enumeration gap (`controls()` dropping WRITE_ONLY/INACTIVE controls entirely)
  is logged separately in Backlog — `set_control` no longer depends on it.

- **GStreamer URI pipeline NULL-on-error teardown (fix/gstreamer-uri-null-on-startup-error)** —
  `UriPipelineHandle::new` in `nokhwa-bindings-gstreamer/src/uri.rs` transitioned the
  pipeline to `Playing` and then had four startup-failure `?`/early-return sites
  (`set_state(Playing)` error, async state-wait error, first-sample timeout,
  `sample_format` parse error) that dropped the local `pipeline` binding **without**
  first transitioning it back to `State::Null`. For a URI source that means the
  underlying RTSP/HTTP socket or `file://` handle stays open until GObject
  finalization, and finalizing a still-Playing pipeline is undefined on some
  GStreamer versions — `UriPipelineHandle::Drop` can't run because `Self` was never
  constructed. The sibling local-capture path (`pipeline.rs`) already does
  `let _ = pipeline.set_state(State::Null)` before each such return; this ports the
  same teardown to the URI path. No behaviour change on success. Compile-verified
  only via CI's `Build & test (input-gstreamer)` Linux/Windows jobs (no GStreamer dev
  libs on the macOS dev box). Verify on a Linux box with libgstreamer installed that a
  bad/unreachable URL (`rtsp://127.0.0.1:1/nope`) leaves no leaked socket/handle after
  the open fails.

- **Core RAWRGB/RAWBGR + GRAY-luma resolution length guards (fix/core-conversion-resolution-guards)** —
  The RAWRGB/RAWBGR arms of every conversion dispatcher in `nokhwa-core/src/frame.rs`
  (`convert_to_rgb`, `convert_to_rgb_buffer`, `convert_to_rgba`, `convert_to_rgba_buffer`,
  `convert_to_luma`, `convert_to_luma_buffer`) validated only `data.len() % 3 == 0` —
  or, in `convert_to_rgb`, nothing beyond that — instead of `data.len() == w*h*3`. A
  payload whose length is a valid multiple of 3 but does not match the frame resolution
  (e.g. an off-by-row backend bug) was silently accepted: `convert_to_rgb` returned the
  raw bytes (and `ImageBuffer::from_raw` then used only the first `w*h*3`), and the RGBA/
  luma paths produced a buffer sized from `data.len()` rather than the resolution —
  yielding a wrong-resolution image with no error. This brings every RAWRGB/RAWBGR arm in
  line with the existing GRAY arms and the `buf_bgr_to_rgb` RAWBGR path, all of which
  already validate against `w*h*bpp`. Separately, `convert_to_luma_buffer`'s GRAY arm
  checked only `dest.len() == data.len()` (not `data.len() == w*h`), unlike its allocating
  sibling `convert_to_luma`; added the resolution guard there too. The new `w*h*3` check
  subsumes the old multiple-of-3 check (any non-multiple-of-3 length also fails it), so no
  valid input is newly rejected. Updated the 8 unit tests that pinned the old
  "not a multiple of 3" message to the resolution-mismatch contract; happy-path
  conversion tests (correctly-sized payloads) unchanged. `nokhwa-core` 399/399 tests pass,
  clippy clean.

- **V4L grayscale `GREY` wire token + `set_format`-while-streaming stale metadata (fix/v4l-grey-fourcc-and-streaming-format-refresh)** —
  Two V4L2 bugs found in a review pass over `nokhwa-bindings-linux-v4l/src/lib.rs`. (1)
  **Grayscale fourcc mismatch:** the V4L2 kernel wire token for grayscale is `GREY`
  (`V4L2_PIX_FMT_GREY`), but the backend used nokhwa-core's canonical `GRAY` everywhere —
  `set_format`'s inline match submitted `b"GRAY"` (rejected by the ioctl) and
  `fourcc_to_frameformat` dropped enumerated `GREY` formats as `None`, so a grayscale
  camera's formats were silently unusable. Fix is scoped to the V4L boundary shims only
  (NOT nokhwa-core's `from_fourcc`/`to_fourcc`, since AVF/GStreamer/MSMF map native
  grayscale constants directly to `FrameFormat::GRAY` and never touch the string table):
  `fourcc_to_frameformat` now maps `GREY → FrameFormat::GRAY`, `frameformat_to_fourcc`
  emits `GREY` for `GRAY`, and `set_format` was changed to use `frameformat_to_fourcc`
  instead of its own divergent inline match. (2) **Stale metadata on `set_format` while
  streaming:** the streaming branch returned `Ok(())` from the `self.open()` success arm
  without ever assigning `self.camera_format` or calling `force_refresh_camera_format()`
  (those lines were unreachable when streaming), so `camera_format()`/`frame()` kept
  reporting the pre-change format. Restructured so the early-return only fires on the
  undo/error path; the success path now falls through to the existing
  `self.camera_format = new_fmt` + `force_refresh_camera_format()` validation for both the
  streaming and non-streaming cases. Updated the byte-equality unit test to encode the
  deliberate `GRAY`→`GREY` divergence and added `grayscale_translates_grey_kernel_token`.
  Compile-/clippy-clean on macOS; the real `internal` mod + these tests are
  `#[cfg(target_os = "linux")]`-gated so they execute only on the Linux CI job + the
  `v4l2loopback` device-test, not locally on macOS.

- **nokhwa-tokio: document `stop()` in-flight frame-loss semantics (docs/tokio-stop-frame-loss)** —
  A review pass over the async surface confirmed `TokioCameraRunner::stop()` drops the async
  receivers before each `spawn_blocking` forwarder's pending `blocking_send` completes, so an item
  a forwarder has pulled from the sync side but not yet handed to the async side is discarded. This
  is correct teardown behaviour for a live stream, but `stop()`'s doc said nothing about it while
  the crate-level docs carefully document drop semantics. Added a paragraph to `stop()` noting the
  in-flight-loss window and pointing callers who need every queued item to drain the receivers to
  `None` first. Doc-only; no logic change; `nokhwa-tokio` tests 6/6 pass, clippy clean. The other
  review findings (event-thread shutdown race under `Overflow::Block`, shutter worker unresponsive
  to `Die` during a blocking `pic_tx.send`) are correct-by-construction edge cases that resolve on
  receiver drop — no code change warranted.

- **AVF planar/stride-aware frame copy (fix/avf-planar-stride-aware-frame-copy)** —
  The capture callback flat-copied the locked `CVPixelBuffer` via
  `GetBaseAddress` + `GetDataSize`, which is wrong for two cases AVFoundation
  actually delivers: (1) **bi-planar 4:2:0** (`420v`/`420f`/`x420`, mapped to
  `FrameFormat::NV12`) is two non-contiguous planes (Y + interleaved CbCr) each
  with its own base address and row stride — the flat copy grabbed only the Y
  plane plus trailing garbage, corrupting every NV12 frame; (2) **packed formats
  with hardware row padding** (stride > width·bpp, common on Apple Silicon) —
  the flat copy dragged padding bytes into the output, which the SIMD/scalar
  decoders (assuming tight-packed rows) then misread, shearing the image. Added
  the plane/stride CoreVideo getters to `ffi.rs` (`CVPixelBufferGetPlaneCount`/
  `IsPlanar`/`BaseAddressOfPlane`/`BytesPerRowOfPlane`/`WidthOfPlane`/
  `HeightOfPlane`/`GetWidth`/`GetHeight`/`GetBytesPerRow`) and rewrote
  `extract_frame_bytes` to repack into the canonical tight-packed layout the
  decoders expect: planar path copies Y (dst stride = plane width) then CbCr
  (dst stride = plane width · 2) row-by-row honoring per-plane source stride;
  packed path copies width·bpp useful bytes per row honoring `GetBytesPerRow`.
  Also dropped the unused `unsafe impl Sync for CaptureCallbackIvars` (kept
  `Send`; the serial GCD queue gives single-threaded *access*, `Cell` is `!Sync`,
  and no shared `&Ivars` crosses threads). Verified 2026-05-20 on macOS (FaceTime
  HD, NV12 only): captured frames are exactly 1920·1080·3/2 = 3110400 bytes
  (tight-packed, no padding, full chroma plane), decode to RGB at correct dims,
  non-degenerate. This camera advertises no YUYV/RGB, so the packed-padding path
  is code-verified only (the canonical-layout output matches what the existing
  decoders already consume on other backends).

- **GStreamer: force pipeline to NULL on startup failure (fix/gstreamer-pipeline-null-on-open-error)** —
  `PipelineHandle::new` returned `Err` on `set_state(Playing)` failure / async-state-wait timeout
  *before* constructing `Self`, so `PipelineHandle::Drop` never ran and the local `Pipeline` binding
  was dropped while still in PAUSED/PLAYING. GStreamer only releases reffed elements (including the
  source = camera device handle) on the NULL transition, so a slow/failing open leaked the device
  handle until GC. Both error paths now call `pipeline.set_state(State::Null)` before returning. Also
  scrubbed a leftover "session-2" dev-phase label from the module doc comment. Compile/clippy-verified
  on macOS (gstreamer-rs checks without system libs). Runtime verification pending on Linux:
  `cargo test --features device-test,input-gstreamer` — confirm a camera that fails to reach PLAYING
  doesn't leave the device busy for subsequent opens.

- **Examples: scrub internal dev-phase jargon from doc comments (docs/examples-drop-session-jargon)** —
  `gstreamer_probe.rs`, `stream_camera.rs`, and `msmf_probe.rs` doc comments referenced internal
  development-sequencing labels ("session-2 streaming path", "session 4 dispatch", "MSMF OBS workflow
  session-2 investigation") that mean nothing to a user reading the examples as copy-paste reference.
  Reworded to describe behavior in user-facing terms (e.g. "local-capture streaming path", "the
  `open()` dispatch"). Doc-comment-only; no logic change.
  Also evaluated W1 (wgpu `bytes_per_row` 256-alignment): confirmed **latent, no fix** — both
  `frame_texture` and `frame_texture_raw` use `queue.write_texture` (CPU→texture), which does not
  enforce `COPY_BYTES_PER_ROW_ALIGNMENT` (that applies only to `copy_buffer_to_texture`). Rounding
  the stride up to 256 would corrupt the upload, so the current unrounded stride is correct.

- **`nokhwa-core` GRAY/NV12 luma buffer dimension validation (fix/core-gray-nv12-luma-dimension-guards)** —
  C1: `convert_to_rgb_buffer` / `convert_to_rgba` / `convert_to_rgba_buffer` GRAY arms were missing
  the `data.len() == w*h` guard present in `convert_to_rgb`'s GRAY arm — added to all three, matching
  the existing message style. C2: `convert_to_luma` GRAY arm returned `Ok(data.to_vec())` with no size
  validation — added the same `w*h` guard. C3: `buf_nv12_extract_luma` skipped the even-dimension check
  present in `buf_nv12_to_rgb`, so `y_size * 3 / 2` truncated for odd dimensions — added the identical
  `!width.is_multiple_of(2) || !height.is_multiple_of(2)` guard with target label "Luma".
  All fixes are pure logic, covered by 11 new unit tests; 399/399 `nokhwa-core` tests pass in CI.

- **MSMF `set_control` forces manual mode when writing explicit value (fix/msmf-set-control-force-manual)** —
  `set_control` was forwarding the device's current auto/manual flag to the driver's `Set` call.
  When the device was in Auto mode, writing an explicit value sent `CameraControl_Flags_Auto` and
  the driver silently ignored the value — Auto→Manual transition was impossible. Now always passes
  `CameraControl_Flags_Manual`. Dropped `self.control(control)?` (only used for the flag) and
  removed unused `CameraControl_Flags_Auto` import. Runtime verification pending — see Open.

- **MSMF `raw_bytes` gratuitous `MFCreateSample` allocation removed (perf/msmf-drop-gratuitous-mfcreatesample)** —
  `raw_bytes` pre-allocated an empty `IMFSample` via `MFCreateSample()` then passed it as the
  `ppsample` outparam to `ReadSample`, which immediately COM-released it and replaced it with the
  captured sample — wasted allocation every frame. Changed `imf_sample` initializer to `None`
  (the correct pattern) and deleted the `MFCreateSample` match block. Removed the now-unused
  `MFCreateSample` import (Clippy would flag it on Windows CI). Compile-checked in CI.

- **MSMF `raw_bytes` Lock/Unlock + stream-end guard (fix/msmf-raw-bytes-unlock-and-stream-end-guard)** —
  Paired every `IMFMediaBuffer::Lock` with a matching `Unlock` on all exit paths (success copy path
  and both early-error paths for null pointer / zero length). Added a stream-flags check in the
  `ReadSample` loop that returns `Err(ReadFrameError)` immediately when
  `MF_SOURCE_READERF_ERROR` or `MF_SOURCE_READERF_ENDOFSTREAM` is set, preventing an infinite
  busy-spin when the camera is unplugged mid-stream. Runtime verification pending — see Open.
- **MSMF hotplug: correct emit-order doc + propagate channel-closed sentinel (fix/msmf-hotplug-doc-and-channel-sentinel)** —
  (F6) Fixed the doc comment on `reconcile_and_emit_with` (and the matching test comment) which claimed
  a rapid re-plug "surfaces as `Disconnected` → `Connected`"; the code emits arrivals first, so the
  correct consumer-visible order is `Connected` → `Disconnected`. Doc-only fix; no behaviour change.
  (F7) `reconcile_and_emit` now returns the bool from `reconcile_and_emit_with` and `wnd_proc` checks it:
  when the consumer has dropped the poller (channel closed), `PostQuitMessage(0)` is posted so the
  message pump exits cleanly instead of processing further `WM_DEVICECHANGE` messages (wasted
  `take_snapshot()` MF enumeration + BTreeMap diff on every hardware event after drop).
  Runtime verification pending on Windows: verify hotplug still emits `Connected` → `Disconnected`
  and that `WM_DEVICECHANGE` after poller drop no longer does wasted work —
  `cargo run --example hotplug_probe` on Windows.

- **V4L Stepwise max-endpoint + zero-step guard (fix/v4l-stepwise-max-endpoint-and-zero-step)** —
  Two bugs in Stepwise frame-size/interval enumeration fixed:
  (1) `V4LCaptureDevice::new()` used an inline `..max_width` (exclusive) + `step_by(step)` + `.zip()` to
  enumerate Stepwise resolutions, missing the max endpoint (causing "Failed to fulfill" for max-res requests),
  panicking on `step == 0`, and truncating the width/height cartesian product to a lockstep zip. Replaced
  with the existing robust `expand_stepwise_resolutions()` helper that is already used in
  `get_resolution_list()`/`compatible_formats()`.
  (2) `expand_frame_interval()` called `step_by(step.step.numerator as usize)` without guarding against
  `step.numerator == 0`; added a `if step.step.numerator == 0 { return vec![]; }` guard before the range.
  Both fixes are unit-test-covered (`expand_frame_interval_stepwise_zero_step_does_not_panic`,
  `expand_stepwise_resolutions_includes_max_endpoint`, `expand_stepwise_resolutions_zero_step_does_not_panic`)
  and will be exercised end-to-end by the Linux `v4l2loopback` CI job on every PR — no separate
  hardware-verification-pending entry needed.

- **AVF active_format() reports negotiated fps from activeVideoMinFrameDuration (fix/avf-active-format-fps)** —
  `active_format()` now reads `activeVideoMinFrameDuration` (the CMTime set by `set_all()`) and
  converts it to fps (timescale/value), falling back to the format maximum only when the CMTime is
  invalid (unset). Previously always returned the max fps from the format's ranges, so a format
  with ranges [1–30] and [1–60] would report 60 fps even after negotiating 30. FaceTime HD has
  single-range formats (one max fps per format), so the bug cannot manifest on this camera — no
  regression (37/37 device-tests pass). Multi-range case is logic-verified.

- **AVF control-layer triple fix: lock leak + inverted exposure flags + exposure POI active arg (fix/avf-control-flags-lock-leak)** —
  `set_all()` now unlocks on format-not-found early return (Bug A). `Gamma`/`Brightness` exposure
  controls now correctly mark `ReadOnly` when custom exposure is unsupported, writable when supported
  (Bug B — flags were inverted). `ExposurePointOfInterest` active arg now uses `exposure_auto ||
  exposure_continuous` instead of the wrong `focus_auto || focus_continuous` (Bug C). Verified 37/37
  device-tests on FaceTime HD.
- **AVF re-open guard + `frame_raw` drain (fix/avf-guard-reopen-and-drain-frame-raw)** —
  Added `if self.is_open() { return Ok(()); }` at the top of `open()` to prevent a second
  `AVCaptureSession` from being created when the device is already streaming (session leak /
  double-feed bug). Added `self.frame_buffer_receiver.try_iter().for_each(drop)` after the
  successful `recv()` in `frame_raw()` to drain stale buffered frames, matching the drain
  already present in `frame()`. Runtime verification pending on hardware — see Open.
- **`nokhwa-core` conversion robustness: u32 overflow fix + length guards (fix/core-conversion-length-guards)** —
  Three complementary fixes to `nokhwa-core` pixel-format conversion functions:
  (F1) `buf_bgr_to_rgb` computed `input_size`/`output_size` as `(width * height * 3) as usize`,
  which overflows `u32` in debug/release for very large resolutions; changed to
  `width as usize * height as usize * 3`. (F2) `convert_to_rgb` RAWRGB arm had no `%3`
  length guard, unlike sibling arms in `convert_to_rgba`/`convert_to_luma`; added the same
  `!data.len().is_multiple_of(3)` check. (F4) `convert_to_luma_buffer` MJPEG arm used a
  `dest.len() < luma.len()` (partial-fill) check while every other arm in that function
  requires `dest.len() != luma.len()` (exact match); tightened to exact match with a
  consistent error message. Two existing tests that pinned the old oversized-dest behavior
  were replaced with a single test covering both too-small and too-large mismatches.
  Skipped F3 (GRAY length guard in `convert_to_rgba`/`convert_to_luma`) — safe by
  construction (`chunks_exact_mut.zip` is bounds-safe) and risks noise without benefit.
  Skipped F5 (dead match arm removal) — no arms were provably unreachable.
  All 388 nokhwa-core unit tests pass in CI.

- **`runner` example feature-gate fix (fix/examples-runner-feature-gate)** —
  Replaced `#![cfg(feature = "runner")]` file-level attribute (which suppresses the entire
  file including `fn main`, producing a silent do-nothing binary when built without `runner`)
  with a `#[cfg(feature = "runner")] fn main()` + `#[cfg(not(feature = "runner"))] fn main()`
  fallback that prints a helpful error message. Matches the `eprintln!` pattern used in
  `hotplug_probe.rs` and `msmf_probe.rs`.

- **SIMD hygiene: `#[target_feature]` on `rgb_to_luma_sse2` + NEON tail NV12 tests (refactor/core-simd-hygiene)** —
  Added missing `#[target_feature(enable = "sse2")]` to `rgb_to_luma_sse2` to match every other
  extension-using SIMD fn in the codebase. Added two NV12 NEON-tail tests (`width=20, height=2`
  and `width=10, height=2`) that exercise the scalar tail path on aarch64 (where NEON processes 16
  px at a time). Both new tests pass on this ARM64 macOS host.

- **`HybridCamera::frame_raw` parity with `StreamCamera` (refactor/session-hybrid-frame-raw)** —
  Added `frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError>` to `HybridCamera` via
  `FrameSource::frame_raw(&mut *self.inner)`. Compile-verified on macOS
  (`cargo build --features input-avfoundation,runner`); not directly hit by
  device-test suite (no `HybridCamera`-capable backend on FaceTime HD, which is
  `StreamCamera`-only). Runtime path is compile-only.

- **`CameraRunner` relay flush loop: replace blocking send with try_send (fix/runner-relay-try-send)** —
  The `DropOldest` relay's producer-disconnect flush loop used a blocking `user_tx.send`, safe only
  because `shutdown()` dropped the user `Receiver` before joining the relay. Replaced with `try_send`
  so the invariant is no longer load-bearing: `Full` → give up (best-effort drain), `Disconnected` → exit.
  Verified 2026-05-20 on macOS (FaceTime HD): 37/37 device-tests pass, relay unit tests including
  `drop_oldest_relay_flushes_non_empty_buffer_on_producer_disconnect` still pass.

- **`decoded_buffer_size` GRAY+alpha fix (fix/core-gray-alpha-buffer-size)** —
  `decoded_buffer_size(true)` returned `w·h·2` for GRAY (pxwidth+1=2) but
  `convert_to_rgba` for GRAY produces a full `w·h·4` RGBA buffer. Fixed the
  `if alpha` branch to force 4 bpp when pxwidth==1. Updated the two tests that
  pinned the wrong value.

- **GRAY→RGB resolution validation + RAWRGB multiple-of-3 check (fix/core-frame-validation-gaps)** —
  `convert_to_rgb` GRAY arm silently processed mismatched-length data (no resolution check).
  `convert_to_rgb_buffer` RAWRGB arm did not check `data.len() % 3 == 0`, unlike the `convert_to_rgba`
  path. Both gaps fixed and covered by unit tests.

- **`CameraRunner` exponential backoff on frame() errors in stream/hybrid workers (fix/runner-backoff-busy-spin)** —
  Added `err_count: u32` tracker to both `spawn_stream` and `spawn_hybrid` worker loops; sleep duration
  is now `poll_interval * 2^min(err_count, 7)` (max ~1.3 s with 10 ms default) instead of a flat 10 ms.
  Resets on the next successful `frame()`. Under `logging` feature: `debug!` on first error, `warn!` on
  power-of-two consecutive failures. Verified 2026-05-20 on macOS (FaceTime HD): 37/37 device-tests pass.

- **`CameraRunner` guard against double-open in `spawn_stream` / `spawn_hybrid` (fix/runner-guard-double-open)** —
  Added `if !cam.is_open()` guard before `cam.open()?` in both spawn paths so that callers who
  already opened the camera don't inadvertently create a second `AVCaptureSession` (resource leak /
  undefined interaction on AVFoundation). Verified 2026-05-20 on macOS (FaceTime HD): 37/37 device-tests pass.

- **AVF `compatible_formats()` max-fps-only fix** — `try_from_format` was pushing both
  `minFrameRate` and `maxFrameRate` endpoints of each `AVFrameRateRange` into `fps_list`,
  but `set_all()` matches solely against `maxFrameRate`. This caused formats like
  `1920x1080 NV12 @15fps` (the `minFrameRate` of a 15–30 fps range) to appear in
  `compatible_formats()` yet be rejected by `set_format()`. Fix: emit only `maxFrameRate`
  from each range; removed the now-dead `range_min_frame_rate` helper. Verified on FaceTime HD:
  `set_format_from_compatible_round_trip` and `negotiated_format_after_set_format_matches` now pass;
  full `device_tests` suite is 37/37.

- **AVFoundation numeric-string `CameraIndex` routing (fix/avf-numeric-string-cameraindex-routing)** —
  `open(CameraIndex::String("0"))` on macOS returned `OpenDeviceError { error: "Device is null" }`
  because the `CameraIndex::String` arm of `AVCaptureDeviceWrapper::new` passed the string directly
  to `device_with_unique_id()` instead of treating it as a positional index. Applied the same fix as
  MSMF PR #387: try `s.parse::<u32>()` at the top of the `String` arm; on success recurse into
  `Self::new(CameraIndex::Index(index))`, otherwise fall through to the existing unique-ID lookup.
  Verified on macOS hardware: `open_numeric_string_routes_to_native_backend` now passes;
  full `device_tests` suite is 35/37 (2 failures are the pre-existing
  `compatible_formats()` false-positive bug tracked in TODO.md → Open → #35).

- **Migrate `NokhwaError::StructureError` construction sites to `NokhwaError::structure()` (refactor/migrate-structure-call-sites)** —
  Migrated 15 struct-literal construction sites across 6 files (nokhwa-core/src/types.rs ×1, nokhwa-bindings-macos-avfoundation/src/capture.rs ×1, nokhwa-bindings-macos-avfoundation/src/device.rs ×2, nokhwa-bindings-windows-msmf/src/lib.rs ×8, nokhwa-bindings-gstreamer/src/uri.rs ×7 constructions (4 pattern-match arms left untouched), nokhwa-bindings-gstreamer/src/pipeline.rs ×1) to the `NokhwaError::structure()` helper added in #434. Static string `.to_string()` calls dropped; `format!(…)` / `why.to_string()` / dynamic expressions left as-is.

- **`NokhwaError::open_device` call-site migration (refactor/migrate-open-device-call-sites)** —
  Migrated all 14 `NokhwaError::OpenDeviceError { … }` struct-literal construction sites across
  v4l (1), avfoundation/capture.rs (1), avfoundation/device.rs (2), msmf (3), gstreamer/uri.rs (2),
  gstreamer/pipeline.rs (4), and nokhwa-core/error_tests.rs (2) to use the new
  `NokhwaError::open_device(device, error)` shorthand constructor from PR #434. Bare `&'static str`
  literals had redundant `.to_string()` dropped; `format!(…)` / `why.to_string()` calls left unchanged.
  Pattern-match arms untouched. Display output identical.

- **AVFoundation `DeclaredClass` + `Ivars` migration (refactor/avf-callback-declared-class)** —
  Replaced the deprecated `ClassBuilder` + `add_ivar` + `get_ivar` / `get_mut_ivar` / `#[allow(deprecated)]`
  pattern in `callback.rs` with the modern `define_class!` macro (`#[ivars = CaptureCallbackIvars]`,
  `unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate`). The `MyCaptureCallback` struct now
  stores the sender pointer in a typed `CaptureCallbackIvars { arc_sender: Cell<*const c_void> }`
  and `AVCaptureVideoCallback.delegate` is a `Retained<MyCaptureCallback>` (ARC-managed) instead of
  a raw `*mut AnyObject`. Session dispatch via `msg_send!` unchanged; `inner()` exposes the raw pointer.
  Runtime-verified 2026-05-20 on macOS (FaceTime HD): 34/37 device-tests pass; the 3 failures are pre-existing AVF bugs unrelated to this refactor — see Open.

- **`NokhwaError::open_device`/`process_frame`/`structure` shorthand constructors (feat/core-add-open-device-process-frame-structure-constructors)** —
  Added three convenience constructors to `nokhwa-core/src/error.rs` following the pattern of
  `get_property`/`set_property` from PR #427. `open_device` and `structure` take two `impl Into<String>`
  fields; `process_frame` takes a `FrameFormat` (non-String) `src` field plus two `impl Into<String>` fields.
  Foundational for follow-up PRs that will migrate call sites across v4l/msmf/avf/gstreamer/core.
  PR: #434

- **Normalize "device not found" error message across backends (refactor/normalize-device-not-found)** —
  Standardized 4 `OpenDeviceError` sites (MSMF ×2, AVF device.rs ×1, GStreamer ×1) to `"device not found"`.
  AVF `capture.rs` `GetPropertyError` ("control not found") left untouched — different semantics.

- **Remove redundant per-item `cast_possible_truncation` allows in V4L crate (refactor/strip-redundant-v4l-cast-allows)** — Deleted 2 per-item `#[allow(clippy::cast_possible_truncation)]` at former lines 90 and 1356 of `nokhwa-bindings-linux-v4l/src/lib.rs`; both were already covered by the crate-level `#![allow(clippy::cast_possible_truncation)]`. The load-bearing `#[allow(clippy::cast_possible_wrap)]` at line 496 (not in the crate-level block) was left untouched.

- **Strip stale `#[allow]` suppressions in MSMF/V4L stubs (refactor/strip-stale-clippy-allows)** —
  Removed 4 stale allows: `missing_errors_doc` / `unused_self` / `needless_pass_by_value` from
  the non-Windows `pub mod wmf` stub (all three already covered crate-wide or didn't fire),
  and `cast_possible_truncation` from V4L's `known_camera_control_to_id` stub (body is
  literal `0`, no cast; covered crate-wide). Kept `must_use_candidate` on `wmf`
  (5 methods fire without it) and `cast_lossless` on `id_to_known_camera_control`
  (`id as u128` fires under `#[deny(clippy::pedantic)]`).

- **AVFoundation property-error helper migration (refactor/avf-use-nokhwa-error-helpers)** —
  Migrated all 23 `NokhwaError::GetPropertyError { … }` / `SetPropertyError { … }` call sites
  in the AVFoundation crate (device.rs: 18, capture.rs: 2, session.rs: 3, types.rs: 1) to the
  `NokhwaError::get_property(…)` / `NokhwaError::set_property(…)` shorthand constructors. Error
  message text preserved verbatim; Display output unchanged.

- **MSMF property-error call-site migration (refactor/msmf-use-nokhwa-error-helpers)** —
  Migrated all 25 `NokhwaError::GetPropertyError { … }` (16) and `NokhwaError::SetPropertyError { … }` (9) struct-literal call sites in `nokhwa-bindings-windows-msmf/src/lib.rs` to the new `NokhwaError::get_property` / `NokhwaError::set_property` shorthand constructors. Bare string literals had redundant `.to_string()` dropped; `format!(…)` / `why.to_string()` / `control.to_string()` calls left unchanged. Display output identical.

- **`nokhwa-core` `process_frame` call-site migration (refactor/core-migrate-process-frame-call-sites)** —
  Migrated all 47 `NokhwaError::ProcessFrameError { src, destination, error }` struct-literal construction sites across `traits.rs` (2), `frame.rs` (28), `wgpu.rs` (2), and `types.rs` (15) to the `NokhwaError::process_frame(src, destination, error)` shorthand constructor. Bare string literals had redundant `.to_string()` dropped; dynamic expressions (`format!(…)`, `why.to_string()`) passed as-is. Pattern-match arms, doc comments, and test fixtures left untouched.

- **Migrate test call sites to `NokhwaError::get_property`/`set_property` helpers (refactor/core-tests-use-nokhwa-error-helpers)** —
  Replaced 2 manual `GetPropertyError { … }` / `SetPropertyError { … }` struct-literal constructions in `error_tests.rs`
  with the shorthand constructors. 2 reference sites in the constructor-validation tests intentionally kept manual (load-bearing).

- **V4L property-error call-site migration (refactor/v4l-use-nokhwa-error-helpers)** —
  Migrated all 26 `NokhwaError::GetPropertyError { … }` / `SetPropertyError { … }` call sites in
  `nokhwa-bindings-linux-v4l/src/lib.rs` to the `NokhwaError::get_property` / `NokhwaError::set_property`
  shorthand constructors from PR #427. Display output is byte-identical; no other files touched.

- **GStreamer `SetPropertyError` → `NokhwaError::set_property` migration (refactor/gstreamer-use-nokhwa-error-helpers)** —
  Migrated all 9 constructor call sites across `controls.rs` (3), `pipeline.rs` (1), and `lib.rs` (5) to use the `NokhwaError::set_property` shorthand introduced in #427. Match-arm patterns in tests unchanged.

- **`NokhwaError::get_property`/`set_property` shorthand constructors (refactor/core-add-property-error-constructors)** —
  Added two convenience constructors to `nokhwa-core/src/error.rs` matching the existing `general`/`open_stream`/`read_frame`/`stream_shutdown` pattern.
  Foundational for upcoming PRs that will mechanically replace ~90 `NokhwaError::GetPropertyError { … }` / `SetPropertyError { … }` call sites.

- **`kcc_to_i32_or_err` helper extraction (refactor/msmf-extract-kcc-to-i32-or-err)** —
  Deduped the identical 5-line `kcc_to_i32(…).ok_or(NokhwaError::SetPropertyError { … })?`
  block shared by `control()` and `set_control()` into a private helper. Error message
  text (`property: "CameraControl"`, `error: "Does not exist"`) preserved verbatim.

- **V4L `query()` flatten + useless_format fix (refactor/v4l-query-flatten-and-fix-useless-format)** —
  Collapsed `Ok({ let x = …; x })` to `Ok(…)` and replaced `format!("{}", …to_string_lossy())` with `.to_string_lossy().into_owned()`. Two load-bearing `#[allow]` attributes unchanged.

- **`CameraRunner::send_cmd` helper extraction (refactor/runner-extract-send-cmd)** —
  Deduped the identical `self.cmd.send(…).map_err(…)` body shared by `trigger` and
  `set_control` into a private `send_cmd` method. Error message text preserved verbatim;
  the test-pinned prefix `"runner thread gone: "` is unchanged.

- **AVFoundation backends (0.14.1–0.14.3 window) runtime-verified** —
  hotplug + open + frame-pull verified 2026-05-20 on macOS (FaceTime HD):
  `cargo test --features device-test,input-avfoundation,runner` → 34/37 pass;
  `cargo run --example hotplug_probe` started cleanly and listened for camera events.
  3 failures are pre-existing AVF bugs unrelated to these releases — see Open.

- **AVFoundation cleanup bundle (refactor/avf-helper-extraction)** —
  `disabled_if_unsupported()` extraction and `pub(crate)` narrowing.
  Runtime-verified 2026-05-20 on macOS (FaceTime HD): 34/37 device-tests pass; the 3 failures are pre-existing AVF bugs unrelated to this refactor — see Open.

- **AVFoundation set_control extract helpers (refactor/avf-set-control-extract-helpers)** —
  `extract_float`/`extract_integer`/`extract_enum`/`extract_boolean`/`verify_or_error`
  helpers replace ~15 inline copies of the same error-wrapping pattern.
  Runtime-verified 2026-05-20 on macOS (FaceTime HD): 34/37 device-tests pass; the 3 failures are pre-existing AVF bugs unrelated to this refactor — see Open.

- **AVF DataPipe/CompressionData removal (refactor/avf-remove-unused-datapipe-and-legacy-comments)** —
  compile-only refactor; no external callers existed.
  Runtime-verified 2026-05-20 on macOS (FaceTime HD): 34/37 device-tests pass; the 3 failures are pre-existing AVF bugs unrelated to this refactor — see Open.

- **`workflow_dispatch` auto-dispatch experiment reverted** (#398,
  #399, then this revert PR) — attempted to skip the
  `RELEASE_PLEASE_TOKEN` PAT by having `release-please.yml`
  `gh workflow run`-trigger the required CI workflows on the release
  PR head (documented exemption from `GITHUB_TOKEN` event-suppression)
  plus `gh pr merge --auto --squash`. The dispatched runs DID attach
  check runs to the PR head SHA, but GitHub's PR merge-state engine
  does NOT count `workflow_dispatch`-sourced check runs toward the
  required-status-checks rollup — the release PR stayed BLOCKED, and
  v0.14.10 only shipped after a manual empty-commit nudge on the
  release branch. Reverted `release-please.yml` to PAT-preferred token
  with `GITHUB_TOKEN` fallback; kept the `workflow_dispatch:` triggers
  on `lint.yml`/`build-matrix.yml`/`test-core.yml` (harmless, useful
  for manual reruns); left `allow_auto_merge=true` on the repo
  (harmless). PAT provisioning re-tracked under Open →
  Infrastructure / CI.
- **Windows GStreamer install cache now actually re-uses across PRs**
  (#392, v0.14.8, 2026-05-13) — the cache added in #391 saved on
  every run but only to `refs/pull/N/merge`, because
  `check-gstreamer-windows.yml` only triggered on `pull_request` /
  `workflow_dispatch`. GitHub Actions restricts cache restores to the
  same ref or the default branch, so every PR's first run reported
  `Cache not found for input keys: gstreamer-windows-1` and re-ran
  the ~250 MB winget install (~4m45s cold). Fix: also trigger on
  `push: branches: [main]` so the post-merge run saves to
  `refs/heads/main`. Verified end-to-end on the v0.14.8 release PR
  (#393): GStreamer Windows job dropped to **2m17s warm vs 4m45s
  cold**, and `gh api .../actions/caches` confirms a 443 MB entry on
  `refs/heads/main`.
- **Windows GStreamer CI promoted to required, with install cache**
  (#391, v0.14.7, 2026-05-13) — `check-gstreamer-windows.yml` went
  green on its first two PR runs (#388 + #389), so dropped job-level
  `continue-on-error` and added an `actions/cache@v4` layer over the
  `%LOCALAPPDATA%\Programs\gstreamer` install. Job added to the
  ruleset required-status contexts so it now blocks merges to `main`.
  Step-level `continue-on-error` stays on the `gstreamer_probe` step
  (it `exit(1)`s without a camera; informational only). Cache scoping
  follow-up shipped in #392 (above).
- **`release-please.yml` prefers `RELEASE_PLEASE_TOKEN` over
  `GITHUB_TOKEN`** (#391, v0.14.7, 2026-05-13) — pushes made by the
  default `GITHUB_TOKEN` don't trigger workflow re-runs, leaving
  every release PR BLOCKED on required status checks until someone
  closes-and-reopens it (we hit this on v0.14.6, v0.14.7, and
  v0.14.8). With a maintainer-supplied PAT the close/reopen dance
  goes away. Falls back to `GITHUB_TOKEN` when the secret is unset.
  Secret provisioning is the maintainer-only follow-up tracked in
  "Open → Infrastructure / CI".
- **Event-driven MSMF hotplug (#173) — live unplug/replug verified on
  real hardware** (2026-05-12) — `hotplug_probe` on the MX Brio:
  unplug printed `Disconnected(MX Brio …)` and replug printed
  `Connected(MX Brio …)` in real time, no poller-Drop hang (the #385
  fix holds). Closes the last "compile-verified only" gap for #173.
- **MSMF teardown crash + hotplug-poller hang fixed; control round-trip
  re-verified** (#384, #385) — `MediaFoundationDevice::drop` released
  `IMFSourceReader` *after* `MFShutdown()`/`CoUninitialize()` (struct
  fields drop after the `Drop::drop` body), which access-violates and
  crashed every open-then-drop flow with `STATUS_ACCESS_VIOLATION` —
  the whole `device_tests` suite on real hardware. Wrapped the field in
  `ManuallyDrop` and dropped it inside the `Drop` body before the MF
  teardown (#384). Separately, `MsmfHotplugPoll::drop` posted `WM_QUIT`
  via `PostThreadMessageW` but the worker's pump filtered `GetMessage`
  to the hidden HWND, so the thread message was never delivered and
  `join()` deadlocked — fixed by passing `NULL` for the hWnd filter
  (#385). The two follow-up failures that surfaced — triplicated
  `compatible_formats()` and numeric-string `CameraIndex` dispatch —
  were fixed in a follow-up (`fix/msmf-format-dedup-and-numeric-string-index`);
  `cargo test --features input-msmf,device-test --test device_tests`
  is now **31/31 green on the MX Brio**.
- **Windows GStreamer local-camera path verified** (session 2, no code
  change) — `gstreamer_probe` on the MX Brio: `DeviceMonitor`
  enumerated 3 sources, opened via `ksvideosrc`, pulled 5× 640×480 NV12
  frames, `controls()` empty (expected on `ksvideosrc`).
  `winget install gstreamerproject.gstreamer` ships the *Complete* MSVC
  variant (headers + `lib/pkgconfig` + `gstreamer-1.0.lib` + 271
  plugins incl. `gstmediafoundation.dll`/`gstwinks.dll`) to
  `%LOCALAPPDATA%\Programs\gstreamer\1.0\msvc_x86_64` — point
  `PKG_CONFIG_PATH` at its `lib\pkgconfig`, add its `bin` to PATH, and
  `cargo build/test --features input-gstreamer` works on Windows.
- **`compatible_fourcc` cross-backend unification** (#194 / #195 /
  #196 / #197 / #198) — fixed silent MSMF truncation to 2 entries
  (#194), unified MSMF/GStreamer to the canonical `collect → sort →
  dedup` shape (#195, #198), added device-test invariants that would
  have caught the truncation bug (#196), and brought AVFoundation up
  to cross-backend hotplug-test parity (#197). All four backends
  (V4L / AVFoundation / MSMF / GStreamer) now return
  `FrameFormat`-`Ord`-sorted, deduplicated lists.
- **V4L Stepwise common-preset enumeration** (`feat/v4l-stepwise-presets`) —
  `get_resolution_list` now also exposes any of {320×240, 640×480,
  800×600, 1024×768, 1280×720, 1280×960, 1920×1080, 2560×1440,
  3840×2160} that fits inside the Stepwise (min..=max) box AND aligns
  to the advertised step. Endpoints (min, max) still always emitted.
  Pure helper + 5 unit tests.
- **Event-driven V4L hotplug** (`perf/v4l-inotify-hotplug`) —
  `inotify(7)` watch on `/dev/` for `IN_CREATE`/`IN_DELETE` replaces
  the 500ms polling loop. Same shape as MSMF #173: worker thread,
  `poll(2)` with 1s timeout for shutdown responsiveness, re-`query()`
  + diff on each kernel notification. Zero steady-state wake-ups.
- **`v4l-loopback` CI fix** (#185) — four compounding bugs silently
  broke the job since the #183 era (job-level `failure` masked by
  run-level `continue-on-error: true`): wrong modules package
  (`-extra` instead of base), stale `modules.dep` on cache hit, DKMS
  `postinst` skipped on cache hit (no `v4l2loopback.ko` for the
  running kernel), `ffmpeg` cached without its transitive shared-lib
  closure (`libblas.so.3`). Verified end-to-end on cold + cache-hit
  runs.
- **`clippy::pedantic` matrix lint CI** (#183) — extended pedantic
  enforcement to `nokhwa-bindings-{linux-v4l, macos-avfoundation,
  windows-msmf}` (previously only `nokhwa-core` + `nokhwa` had it).
  `lint.yml` expanded to a 3-OS matrix; ruleset required-status
  contexts updated to `Clippy (linux/windows/macos)`. Removed
  `required_signatures` from the ruleset (release-please bot can't
  sign; web-flow auto-signs squash merges so verification is
  preserved).
- **0.14.3** (2026-04-22) — GStreamer sessions 3/4/5 + OpenCV removal.
- **0.14.2** (2026-04-21) — MSMF / V4L / AVFoundation hotplug, OpenCV
  IP-camera re-open fix, MSMF OBS spike docs, GStreamer session 1/2,
  UVC session 1/2a then pre-release removal.
- **Event-driven MSMF hotplug** (#173, post-0.14.3) —
  `RegisterDeviceNotificationW(KSCATEGORY_VIDEO_CAMERA)` + hidden
  `HWND_MESSAGE` window + `WM_DEVICECHANGE` pump. Zero steady-state
  wake-ups.
- **V4L + test-core apt caches** (#175, #176) — cache `.deb` archives
  across CI runs; ~90 s → ~10 s on v4l-loopback, ~30 s → ~5 s on
  check-gstreamer.
- **CLAUDE.md rules**: (1) never `cargo publish` to crates.io (fork);
  (2) prefer `winget` over `choco`, direct MSI only where winget lacks
  the variant (e.g. GStreamer `-devel`).
