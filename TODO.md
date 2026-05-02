# TODO

Working list. Short lines only — rationale + implementation notes live
in `CHANGELOG.md`, PR descriptions, and commit messages.

## Open

### Runtime verification pending (compile-verified only)

- [ ] **Event-driven MSMF hotplug (#173)** — reconnect the MX Brio over
  USB and run `cargo run --features input-msmf --example hotplug_probe`;
  unplug/replug should print `Connected(…)` / `Disconnected(…)` in real
  time.
- [ ] **AVFoundation backends (0.14.1–0.14.3 window)** — hotplug + open +
  frame-pull have only the `Build (macos)` compile check. Needs a run on
  the self-hosted `macos-camera` runner.
- [ ] **Windows GStreamer local-camera path (session 2)** — Windows
  runtime exercised with `file://` URLs via `uridecodebin` only;
  `DeviceMonitor` + `ksvideosrc`/`mfvideosrc` against a live USB camera
  still needs a manual run.
- [ ] **MSMF control round-trip** verified 2026-04-20 on MX Brio; re-run
  if the trait surface changes.

### Infrastructure / CI

- [ ] **Windows GStreamer CI** blocked on `gstreamer.freedesktop.org`'s
  `go-away` JS challenge (PR #174 closed). Paths forward: (a) private
  artifact mirror the CI can pull from, (b) wait for `winget` to gain a
  `-devel` manifest, (c) self-hosted Windows runner with GStreamer
  pre-installed. `Build (windows)` matrix still exercises `input-msmf`
  so no regression.
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

### Perf follow-ups (correctness already fine)

- [ ] AVFoundation event-driven hotplug via `IOKit` matching
  notifications. Current impl is 500ms polling. (Deferred to a
  different host with macOS access.)

### Backlog

- [ ] **V4L Stepwise frame-size enumeration.** Currently
  `get_resolution_list` exposes only the (min, max) endpoints of a
  `FrameSizeEnum::Stepwise` advertisement. A full enumeration would
  walk `(min..=max).step_by(step)` per axis, but unbounded steps
  (e.g. 1×1) produce hundreds of synthetic resolutions; a sensible
  upper bound + a "common preset" filter (640×480, 1280×720,
  1920×1080) is the practical fix. Drivers still accept arbitrary
  intermediate resolutions via `set_format` so this is a
  surface-quality issue, not correctness.
- [ ] **WASM / browser backend.** Blocked on five design decisions, no
  active consumer:
  - interop library (`tsify` vs `serde-wasm-bindgen` vs hand-rolled)
  - `ApiBackend::Custom(String)` representation in JS
  - frame transport (`Uint8Array` / `OffscreenCanvas` / `ImageBitmap`)
  - `NokhwaError` → JS Error translation
  - browser capture API (`getUserMedia` + `MediaStreamTrackProcessor` vs
    `ImageCapture`)
- [ ] Expand platform integration tests in `tests/device_tests.rs`.
  Already covers query, multi-frame streaming, control enumeration,
  `CameraRunner` smoke, control round-trip. Linux is auto-covered on
  every PR via `v4l2loopback`.

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
