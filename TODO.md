# TODO

Working list. Short lines only — rationale + implementation notes live
in `CHANGELOG.md`, PR descriptions, and commit messages.

## Open

### Runtime verification pending (compile-verified only)

- [ ] **Event-driven MSMF hotplug (#173) — live unplug/replug** —
  reconnect the MX Brio over USB and run `cargo run --features
  input-msmf --example hotplug_probe`; unplug/replug should print
  `Connected(…)` / `Disconnected(…)` in real time. (The Poller-Drop
  deadlock that previously made this example hang is fixed — #385; the
  automated `msmf_hotplug_take_and_steady_state` test passes. Only the
  live human observation remains.)
- [ ] **AVFoundation backends (0.14.1–0.14.3 window)** — hotplug + open +
  frame-pull have only the `Build (macos)` compile check. Needs a run on
  the self-hosted `macos-camera` runner.

### Infrastructure / CI

- [ ] **Windows GStreamer CI** — local dev install now works via
  `winget install gstreamerproject.gstreamer` (the Complete MSVC
  variant, see the verified local-camera-path note above), so a
  Windows CI job is newly feasible: install via winget, export
  `PKG_CONFIG_PATH`, run `cargo build/test --features input-gstreamer`.
  The old blocker (`gstreamer.freedesktop.org`'s `go-away` JS
  challenge breaking direct MSI downloads, PR #174 closed) is sidestepped
  because winget's installer URL handling clears the challenge.
  Remaining cost: GStreamer plugins make for a large download — gate
  the job behind a cache, or accept the ~2-3 min install. `Build
  (windows)` matrix still exercises `input-msmf` regardless.
- [ ] **MSMF device-test coverage on a GH-hosted `windows-latest`**
  runner. OBS virtualcam spike (`msmf-obs-virtualcam.yml`) is abandoned
  — OBS is a DirectShow filter, invisible to `MFEnumDeviceSources`.
  Remaining candidate paths:
  - Windows 11 Camera Extension sample (smourier/VCamSample) — requires
    a code-signing certificate GH Actions can't provide.
  - Ship a minimal Rust MF source in the test harness — feasible but
    ~500 LOC `unsafe` `windows` FFI; feasibility of userspace
    `IMFActivate` appearing in `MFEnumDeviceSources` is unverified.
  - Self-hosted Windows runner with a USB webcam (same pattern as
    `macos-camera`).
  - Accept the gap — current state; `msmf-obs-virtualcam.yml` stays as
    a diagnostic harness (`workflow_dispatch`-only,
    `continue-on-error: true`).

### Backlog

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
- **OBS virtualcam MSMF CI spike** (abandoned 2026-04-21) — OBS
  virtualcam is a DirectShow filter; `MFEnumDeviceSources` and
  DirectShow are disjoint enumeration namespaces. No amount of OBS
  configuration bridges that. `msmf-obs-virtualcam.yml` kept as a
  diagnostic harness, `workflow_dispatch`-only.
- **macOS GH-hosted virtual camera** — not feasible. Modern vcams need
  system extensions codesigned + notarized + installed from
  `/Applications`; GH-hosted macOS runners have no Apple Developer
  credentials. AVFoundation CI coverage = self-hosted `macos-camera`.
- **Network/IP camera backend** — superseded by GStreamer session 5's
  URL path. `CameraIndex::String("rtsp://…")` / `https://…` / `file://…`
  dispatches through `uridecodebin`.

## Shipped recently (for context)

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
