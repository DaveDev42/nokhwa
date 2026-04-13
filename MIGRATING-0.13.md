# Migrating from nokhwa 0.12 to 0.13

0.13.0 is a structural refactor release. The single `CaptureBackendTrait`
has been split into capability-based traits, and the user-facing `Camera`
and `CallbackCamera` types have been replaced by `CameraSession` (with an
`OpenedCamera` enum) and `CameraRunner`.

## Why

The old trait assumed all cameras worked like webcams: one continuous
stream, `open → frame → stop`. This did not fit DSLR-class cameras where
shutter trigger and image arrival are decoupled in time. 0.13.0 makes it
possible for external backends (e.g. `canon-edsdk-nokhwa`) to plug in as
custom backends with a shutter-capture model.

## API changes

### Opening a camera

```rust
// 0.12
use nokhwa::Camera;
let mut cam = Camera::new(index, requested_format)?;

// 0.13
use nokhwa::{CameraSession, OpenedCamera, OpenRequest};
let opened = CameraSession::open(index, OpenRequest::any())?;
let mut cam = match opened {
    OpenedCamera::Stream(c) => c,
    _ => panic!("expected stream camera"),
};
```

### Stream lifecycle

| 0.12                   | 0.13                   |
| ---------------------- | ---------------------- |
| `cam.open_stream()`    | `cam.open()`           |
| `cam.frame()`          | `cam.frame()`          |
| `cam.frame_timeout(d)` | `cam.frame_timeout(d)` |
| `cam.frame_raw()`      | `cam.frame_raw()`      |
| `cam.stop_stream()`    | `cam.close()`          |

### Format negotiation

Dropped methods: `refresh_camera_format`, `resolution`, `frame_rate`,
`frame_format`, `set_resolution`, `set_frame_rate`, `set_frame_format`,
single-control `camera_control(id)`.

Use `negotiated_format()` for the current `CameraFormat` (resolution,
fourcc, framerate combined) and `set_format(CameraFormat)` to change it.
Use `controls()` for the full control list.

### Threaded capture

`CallbackCamera` is removed. Use `CameraRunner` (feature `runner`):

```rust
// 0.12
use nokhwa::CallbackCamera;
let cam = CallbackCamera::new(idx, fmt, |buf| { /* ... */ })?;

// 0.13
use nokhwa::{CameraRunner, CameraSession, OpenRequest, RunnerConfig};
let opened = CameraSession::open(idx, OpenRequest::any())?;
let runner = CameraRunner::spawn(opened, RunnerConfig::default())?;
for buf in runner.frames().unwrap().iter() { /* ... */ }
```

The callback model is replaced by `std::sync::mpsc::Receiver`. Apps
needing `async`/`tokio` integration should wrap `recv` calls in
`spawn_blocking` for now; an async runner is on the 0.14.0 roadmap.

### Custom backends

Replace `impl CaptureBackendTrait for MyBackend` with up to four
capability impls plus a `nokhwa_backend!` declaration:

```rust
impl CameraDevice   for MyBackend { /* ... */ }
impl FrameSource    for MyBackend { /* ... */ } // if applicable
impl ShutterCapture for MyBackend { /* ... */ } // if applicable
impl EventSource    for MyBackend { /* ... */ } // if applicable

nokhwa::nokhwa_backend!(MyBackend: FrameSource, ShutterCapture);
```

The macro emits the internal `AnyDevice` impl that `OpenedCamera` uses to
dispatch into `StreamCamera` / `ShutterCamera` / `HybridCamera` based on
declared capabilities.

### Feature flags

| 0.12              | 0.13     |
| ----------------- | -------- |
| `output-threaded` | `runner` |

## Disabled in 0.13.0

- `input-opencv` backend: pending migration. The feature definition is
  preserved but enabling it triggers a `compile_error!` until the
  backend is adapted to the new traits. Track progress in `TODO.md`.
- **V4L via `CameraSession::open`**: on Linux, `CameraSession::open`
  returns a `NokhwaError::general` in 0.13.0. The `V4LCaptureDevice<'a>`
  lifetime parameter cannot be unified with `'static` (required for
  `dyn AnyDevice`) without an `unsafe` transmute of the `MmapStream`
  handle; the reworked dispatch path will ship in **0.13.1** after Linux
  CI validation. Users can still construct `V4LCaptureDevice` directly
  via the `nokhwa-bindings-linux-v4l` crate in the meantime.

## `RunnerConfig` fields

`RunnerConfig` has been trimmed in 0.13.0. Only three fields remain:

- `poll_interval` (was `tick`): worker poll interval.
- `event_tick`: event-poll timeout.
- `shutter_timeout` (new): replaces the previously hard-coded 200 ms
  timeout on `take_picture(…)`; defaults to 5 s. If your shutter code
  relied on the short fast-fail behaviour, set
  `shutter_timeout = Duration::from_millis(200)` explicitly.

The vestigial `frames_capacity` / `pictures_capacity` / `events_capacity`
/ `overflow` fields (and the `Overflow` enum) were removed because the
underlying `std::sync::mpsc::channel` is unbounded; bounded channels +
overflow policy are planned for 0.14. If you were constructing a
`RunnerConfig` manually, use the `Default` impl or the three remaining
fields.

## `CameraSession`

`CameraSession` is now a unit struct; the no-op `CameraSession::new(req)`
constructor was removed. Open a camera directly via
`CameraSession::open(index, req)`.

## Windows / MSMF COM apartment change

0.13 initialises COM with `COINIT_MULTITHREADED` (MTA) instead of the
previous `COINIT_APARTMENT_THREADED` (STA). This matches the `unsafe impl
Send for MediaFoundationCaptureDevice` assertion that `CameraRunner` now
actually exercises. If your host application (typical Windows GUI main
threads) has already called `CoInitializeEx(.., COINIT_APARTMENT_THREADED)`
on the same thread before touching nokhwa, `initialize_mf` will return
`RPC_E_CHANGED_MODE`. Workaround: call nokhwa from a thread that has not
been initialised, or from a fresh worker thread.
