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

- [ ] **MSMF handles driver-initiated mid-stream media-type changes (fix/msmf-mediatype-changed)** —
  In `nokhwa-bindings-windows-msmf`, `raw_bytes()`'s `ReadSample` loop only checked the
  `ERROR` / `ENDOFSTREAM` stream flags. When the driver spontaneously renegotiates the media
  type mid-stream it sets `MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED` and delivers the sample
  already in the *new* format, but the cached `device_format` (which the caller uses to tag the
  `Buffer`'s resolution/format) stayed stale — so the frame after a format change was mislabeled,
  corrupting downstream decode (garbage for planar formats, decoder failure for MJPEG). Fix:
  detect the flag in the loop and call `format_refreshed()` to re-read the negotiated type, and
  in `capture.rs::frame()` snapshot `negotiated_format()` *after* `raw_bytes()` returns (so the
  tag matches the bytes). Also removed a redundant `self.device_format = format` write in
  `set_format` (the immediately-following `format_refreshed()?` is the single source of truth;
  the pre-write left an unconfirmed value cached if the refresh failed). Windows-gated
  (`cfg(all(windows, not(docs-only)))`), compile-checked off-Windows via the stub only. Verify on
  a Windows box with a UVC camera that normal streaming is unaffected; the format-change path
  itself needs a camera/driver that renegotiates mid-stream (rare) to exercise directly.

- [ ] **V4L `frame()`/`frame_raw()` truncate to `bytesused` (fix/v4l-frame-bytesused)** —
  In `nokhwa-bindings-linux-v4l/src/lib.rs`, `frame()` and `frame_raw()` handed the full mmap
  buffer slice from `MmapStream::next()` downstream. The v4l crate sizes each arena buffer to
  `v4l2_buf.length` (the driver's max image size) and returns that whole slice; the actual frame
  length lives in `Metadata.bytesused`, which we ignored. For raw formats (YUYV/NV12)
  `bytesused == length` so it was invisible, but for **MJPEG** the buffer is far larger than the
  encoded frame, so consumers received the JPEG bytes plus a tail of stale padding — corrupting
  strict decoders and any `frame_raw()` consumer. Fix: clamp `data` to `&data[..bytesused]`
  (with `.min(data.len())` against a driver over-reporting). CI's `v4l2loopback` test pattern is
  raw YUYV (`bytesused == length`), so it confirms no regression on the raw path but does **not**
  exercise the compressed-frame difference — verify on real hardware with an MJPEG webcam that
  `frame_raw()` length equals the JPEG size and that MJPEG decode succeeds.

- [ ] **MSMF Exposure/Iris/Focus report their min/max range (fix/msmf-cc-ranged-controls)** —
  In `nokhwa-bindings-windows-msmf/src/lib.rs`, `Exposure`, `Iris`, and `Focus` were mapped to
  `MFControlId::CCValue`, whose `control()` arm built a `ControlValueDescription::Integer { value,
  default, step }` and silently discarded the `min`/`max` that `query_camera_control` had already
  fetched via `IAMCameraControl::GetRange`. So a caller querying the valid range for these three
  ranged `IAMCameraControl` properties got a descriptor with no bounds (and any `verify_setter`
  built on it was useless), even though Pan/Tilt/Zoom — identical `IAMCameraControl` properties —
  correctly used `CCRange`. Fix: map all three to `CCRange` so the reported descriptor is
  `IntegerRange` with the real bounds. Since `query_camera_control` already calls `GetRange` + `Get`
  for both variants and `set_control` treats them identically (`IAMCameraControl::Set`), this only
  changes the *reported* descriptor — no FFI-call change. The now-unused `CCValue` variant and its
  `control()`/`set_control` arms were removed; the `kcc_to_i32_maps_every_standard_control` mapping
  test was updated in lockstep. Windows-gated code (`cfg(all(windows, not(docs-only)))`), so it is
  compile-checked off-Windows only (stub + docs-only build clean locally). Verify on a Windows box
  with a UVC cam that `controls()` now reports non-degenerate min/max for Exposure/Focus and that
  setting a clamped value still works: `cargo test --features device-test,input-msmf,runner`.

- [ ] **V4L `set_format` tears down the live stream before re-negotiating (fix/v4l-set-format-stream-teardown)** —
  `set_format` in `nokhwa-bindings-linux-v4l/src/lib.rs` issued `VIDIOC_S_FMT` / `VIDIOC_S_PARM`
  while a stream was still active and then called `self.open()`, which allocates the new
  `MmapStream` (`REQBUFS` + `mmap`) *before* dropping the old one. V4L2 rejects `S_FMT`/`REQBUFS`
  on a streaming device with EBUSY, and requesting new buffers while the old arena is still mapped
  is a use-after-stream hazard. Now: snapshot prev format/params, `self.close()` the live stream
  first (its `Drop` issues `VIDIOC_STREAMOFF` + munmap), set the new format/params, then re-open;
  the undo path restores the prior format/params and re-opens so a failed re-negotiation does not
  leave the device silently closed. The Linux code path is `#[cfg(target_os = "linux")]`, so it is
  compile-checked off-Linux only; verified by the new cross-platform device test
  `set_format_while_streaming_reacquires_stream` (open → frame → set_format(other) → frame), which
  passed locally against a real Mac webcam on the AVFoundation backend. Verify on Linux via CI's
  v4l2loopback `device-test` job (and on a physical UVC cam if available) that a streaming-time
  format change keeps delivering frames at the new resolution.

- [ ] **GStreamer URI pad-added falls back to `query_caps` (fix/gstreamer-uri-pad-caps-fallback)** —
  the `uridecodebin` `pad-added` callback in `nokhwa-bindings-gstreamer/src/uri.rs` filtered
  video pads using only `new_pad.current_caps()`. On a freshly-added dynamic pad `current_caps()`
  can still be `None` until the first caps event flows, so a valid decoded video pad could be
  silently declined — leaving `videoconvert` unlinked and the open path timing out after the 10s
  `FIRST_SAMPLE_TIMEOUT` with a spurious "stream unreachable" error. Now falls back to
  `new_pad.query_caps(None)` when `current_caps()` is `None`, then checks `structure(0)` for a
  `video/` mime prefix as before (audio pads are still declined). Behind `feature = "backend"`,
  which needs system GStreamer libs absent on the macOS dev box, so only compile-checked in CI
  (`Feature check (input-gstreamer)` + `Build & test (input-gstreamer, …)`). Verify against a
  real URL source — an `rtsp://` cam and a `file://*.mp4` — that the first sample arrives without
  the timeout, and that an audio-only URL still errors cleanly rather than linking an audio pad.

- [ ] **AVF callback drops unknown formats + checks lock return (fix/avf-callback-unknown-format-and-lock-check)** —
  The sample-buffer callback did `raw_fcc_to_frameformat(pixel_format).unwrap_or(FrameFormat::YUYV)`,
  so any unrecognised pixel format was forced through the YUYV (2 bpp) repack path, producing a
  wrong-length garbage frame instead of being dropped. It also ignored the `CVReturn` from
  `CVPixelBufferLockBaseAddress` and called `CVPixelBufferUnlockBaseAddress` unconditionally — if the
  lock failed, plane pointers were read while unlocked and an unmatched Unlock was issued (UB per
  Apple docs). Now: unknown formats `return` early (drop the frame); a non-zero lock `CVReturn`
  returns before any plane read and skips the Unlock. Compile + clippy verified on macOS. Verify on
  hardware that normal capture is unaffected and that a format-renegotiation glitch drops frames
  rather than emitting corrupt ones: `cargo test --features device-test,input-avfoundation,runner`.

- [ ] **AVF callback owns the `Arc<Sender>` strong count (fix/avf-callback-arc-ownership)** —
  `AVCaptureVideoCallback::new` stored `Arc::as_ptr(buffer)` (a *borrowed* pointer, no
  refcount bump) and the GCD sample-buffer callback reconstructed it with `Arc::from_raw`
  + `mem::forget`. The `Arc<Sender>` was kept alive only by the sibling `fbufsnd` field on
  `AVFoundationCaptureDevice`, which Rust drops *after* the delegate. Because
  `AVCaptureSession::stopRunning` does not guarantee an already-dispatched callback block
  on the serial GCD queue has finished, a callback could `Arc::from_raw` the `Sender` after
  `fbufsnd` was dropped → teardown use-after-free. Fixed by handing the delegate an owned
  count via `Arc::into_raw(Arc::clone(buffer))` and releasing it in a `Drop for
  MyCaptureCallback` impl (objc2 runs it from ObjC `dealloc`, after the last GCD reference is
  gone). Compile + clippy verified on the macOS dev box. Verify on hardware that open →
  stream → drop (and rapid open/close cycles) leak no `Sender` and never fault during
  teardown: `cargo test --features device-test,input-avfoundation,runner`.

- [ ] **MSMF capture timestamp accepts PTS 0 + overflow-safe (fix/msmf-capture-timestamp-zero-and-overflow)** —
  `raw_bytes` computed `capture_ts` under `if sample_time_100ns > 0`, dropping a legitimate
  first-frame presentation time of exactly `0` (returned `None`). The inner
  `u64::try_from(sample_time_100ns).unwrap_or(0)` was also dead (the `> 0` guard already
  excluded negatives) and the `* 100` could wrap in release. Replaced with a single
  `u64::try_from(...).ok().and_then(checked_mul(100))…` chain: PTS `>= 0` maps to a stamp,
  negative PTS → `None`, and the 100ns→ns scale uses `checked_mul`. Compile-checked in CI
  (Windows). Verify on Windows hardware that frame timestamps are monotonic and that the
  first frame carries a stamp: `cargo test --features device-test,input-msmf,runner`.

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

- [ ] **AVF drops every MJPEG frame (`frame()` hangs on MJPEG-only macOS cameras).** In
  `nokhwa-bindings-macos-avfoundation/src/callback.rs`, the sample-buffer delegate calls
  `CMSampleBufferGetImageBuffer` (line ~267) and returns early when it is null (line ~269).
  For compressed codecs (`kCMVideoCodecType_JPEG`, requested for `FrameFormat::MJPEG` in
  `session.rs:115`) AVFoundation delivers data via a `CMBlockBuffer`, not a `CVImageBuffer`,
  so `GetImageBuffer` is always null and **every MJPEG frame is silently discarded** — a
  consumer that negotiated MJPEG blocks forever in `frame()`. The fix is to detect the
  null-image-buffer case and extract the JPEG bytes via `CMSampleBufferGetDataBuffer` +
  `CMBlockBufferCopyDataBytes` instead. Deferred: the maintainer's Mac camera offers only
  NV12 (verified via `nokhwactl list-properties 0 compatibleformats` → NV12 at 7
  resolutions, no MJPEG), so this path is unreachable here and cannot be runtime-verified.
  Needs an external UVC webcam that exposes an MJPEG format on macOS.

- [ ] **AVF maps 10-bit `x420` to 8-bit `NV12` (latent corruption on 10-bit-only cameras).** In
  `nokhwa-bindings-macos-avfoundation/src/util.rs:18`,
  `kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange` (`x420`) is mapped to
  `FrameFormat::NV12`. `x420` packs each 10-bit sample into a 16-bit word (2 bytes/column),
  but `extract_frame_bytes` (`callback.rs:194`) copies the Y plane at `dst_stride = plane_w`
  (1 byte/column), truncating each luma row to half its real byte width — and even a
  corrected byte count would feed 10-bit data to the 8-bit NV12 decoder. `output_set_frame_format`
  requests 8-bit (`session.rs:117`), so `x420` only arrives if a camera offers *only* the
  10-bit format. Safer fix is to drop the `x420 → NV12` mapping entirely (so negotiation
  doesn't claim support it can't honor) rather than half-implement 10-bit unpacking.
  Deferred: unreachable on the maintainer's NV12-8-bit camera; needs a 10-bit-only camera to
  verify the behavior change does not regress real hardware.

- [ ] **AVF `ExposureDuration` control assumes a uniform `CMTime` timescale.** In
  `nokhwa-bindings-macos-avfoundation/src/device.rs`, the `ExposureDuration` control
  (mapped to `KnownCameraControl::Gamma`) builds its `IntegerRange` from raw
  `CMTime.value` ticks: `min`/`max` from `format.minExposureDuration()` /
  `maxExposureDuration()` (lines ~933-934), `value`/`default` from
  `device.exposureDuration()` / `AVCaptureExposureDurationCurrent` (lines ~935-937).
  On `set_control` (lines ~1191-1196) the new `CMTime` reuses
  `current_duration.timescale`. If the format-reported min/max `CMTime`s ever carry a
  different `timescale` than the device's live `exposureDuration`, the tick counts are
  not comparable and a write at the reported "min" would apply the wrong physical
  duration. In practice Apple reports all of these for the same active format with a
  uniform high timescale (typically 1e9), so this has not been observed to misbehave —
  but the code does not normalize, so it is a latent unit-mixing hazard. Proper fix:
  expose the range in a canonical unit (e.g. nanoseconds) or normalize all three
  `CMTime`s to one timescale before ranging/writing. Deferred — needs hardware
  inspection of the actual reported timescales across formats before changing behavior
  (`cargo test --features device-test,input-avfoundation,runner`).

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

- [ ] **GStreamer `pull_frame` does not validate the mapped buffer size.**
  In `nokhwa-bindings-gstreamer/src/pipeline.rs::pull_frame` (line ~207),
  the readable map is handed straight to `Buffer::new(self.format.resolution(),
  map.as_slice(), self.format.format())` with no check that `map.len()` matches
  the expected wire size for the negotiated format. A short or oversized buffer
  (e.g. a partially-decoded frame, a caps/stride mismatch) is currently caught
  downstream by the `nokhwa-core` conversion-length guards (shipped in #476),
  which reject it at `into_rgb()`/`into_luma()` time. A defensive size check at
  the source would fail earlier with a clearer GStreamer-scoped `ReadFrameError`
  rather than surfacing as a conversion error later. Proper fix wants a shared
  wire-size helper (the V4L2 and AVFoundation paths compute the same expectation
  independently) rather than another inline `width*height*bpp` in this one spot;
  deferred until that helper exists so the three backends stay consistent.

- [ ] **GStreamer V4L2 control restart briefly runs two `v4l2src` on the same
  device.** In `nokhwa-bindings-gstreamer/src/lib.rs::set_control` (line ~327),
  applying a `V4l2Cid` control to an already-open local pipeline builds the
  replacement `PipelineHandle` (`PipelineHandle::start(...)`) *before* assigning
  it to `self.pipeline`, so for a brief window two pipelines exist, each with its
  own `v4l2src` element opening the same `/dev/videoN`. This ordering is
  **intentional** (the keep-stream-alive design noted in the inline comment: on
  start failure the old pipeline keeps running and the device stays usable), so
  this is a recorded known-limitation, not a bug to "fix" by dropping the old
  pipeline first — that would regress the failure-path guarantee. Some V4L2
  drivers permit multiple opens of the same node; others (single-open UVC
  gadgets) may reject the second `v4l2src` and fail the restart, in which case
  the staged control still lands on the next `open()`. Revisit only if a driver
  is found that mishandles the transient double-open in a way the failure path
  doesn't already cover (`cargo test --features device-test,input-gstreamer`).

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
