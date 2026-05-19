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

- [ ] **MSMF CAMERA_REFCNT atomic RMW fix (fix/msmf-camera-refcnt-atomic-rmw)** —
  `fetch_add`/`fetch_sub` 전환. Windows에서 멀티스레드 동시 open/close가
  refcount 정확성을 유지하는지 확인. 단일 스레드 동작은 변경 없음.
  `cargo test --features device-test,input-msmf,runner` on Windows hardware.

- [ ] **MSMF INITIALIZED atomic CAS fix (fix/msmf-initialized-atomic-cas)** —
  `load+store` → `compare_exchange` 전환 (#411과 동일 클래스). 두 스레드가
  동시에 `initialize_mf`를 호출해도 `CoInitializeEx + MFStartup`이 한 번만
  실행되고, 마찬가지로 `de_initialize_mf`도 한 번만 `MFShutdown +
  CoUninitialize` 실행. 단일 스레드 흐름은 변경 없음. Init 실패 시
  INITIALIZED를 false로 되돌려 다음 호출이 재시도 가능하도록 함.
  Windows hardware에서 `cargo test --features device-test,input-msmf,runner`로
  단일 스레드 open/close 시나리오가 여전히 동작하는지 확인.

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

- [ ] **AVFoundation backends (0.14.1–0.14.3 window)** — hotplug + open +
  frame-pull have only the `Build (macos)` compile check. Run
  `cargo test --features device-test,input-avfoundation,runner` on a
  Mac with a webcam (the self-hosted `macos-camera` runner is one such
  machine; a local run is equally valid), plus `cargo run --example
  hotplug_probe` for the hotplug path.
- [ ] **MSMF COM service helper refactor (refactor/msmf-com-service-helper)** —
  `get_camera_control_services()` consolidation has only the `Build (windows)`
  compile check. Verify control read/write on physical Windows hardware
  (`cargo test --features device-test,input-msmf,runner` on a Win box with a webcam).
- [ ] **AVFoundation cleanup bundle (refactor/avf-helper-extraction)** —
  `disabled_if_unsupported()` extraction and `pub(crate)` narrowing have only
  `Build (macos)` + clippy compile coverage. Verify on Mac with a webcam:
  `cargo test --features device-test,input-avfoundation,runner` + `cargo
  run --example hotplug_probe`.
- [ ] **AVFoundation set_control extract helpers (refactor/avf-set-control-extract-helpers)** —
  `extract_float`/`extract_integer`/`extract_enum`/`extract_boolean`/`verify_or_error`
  helpers replace ~15 inline copies of the same error-wrapping pattern. Compile-checked
  via `Build (macos)` + clippy only. Verify per-control round-trip on Mac hardware:
  `cargo test --features device-test,input-avfoundation,runner`. Specifically exercise
  Brightness/Gamma/WhiteBalance/Sharpness/PowerLineFrequency/Focus/Exposure/Iris/Saturation
  `set_control()` paths to confirm error messages and behavior are unchanged.

- [ ] **MSMF FIRST_VIDEO_STREAM const unification (refactor/msmf-unify-first-video-stream-const)** —
  로컬 상수를 windows-rs import로 통일. 값 동일하지만 Windows에서 enum/format 열거가
  변함없이 동작하는지 확인: `cargo test --features device-test,input-msmf,runner` on
  a Windows box with a webcam attached.

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

- **AVF DataPipe/CompressionData removal (refactor/avf-remove-unused-datapipe-and-legacy-comments)** —
  compile-only refactor; no external callers existed. Verify AVF capture
  still works normally on macOS with a webcam:
  `cargo test --features device-test,input-avfoundation,runner`.

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
