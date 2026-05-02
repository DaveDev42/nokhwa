# threaded-capture

Demonstrates background-thread capture via `CameraRunner` (the post-0.13
replacement for the removed `CallbackCamera<F>` type).

## What it demonstrates

- `open(index, OpenRequest::any())` returns an `OpenedCamera`, which is
  handed to `CameraRunner::spawn(opened, RunnerConfig::default())`.
- The runner owns a background thread that drives `frame()` in a loop
  and forwards each `Buffer` over a bounded `std::sync::mpsc::Receiver`
  exposed by `runner.frames()`.
- The main thread `recv_timeout`s ten frames, wraps each `Buffer` into
  the typed `Frame<Mjpeg>`, and decodes via
  `frame.into_rgba().materialize()` into an `image::ImageBuffer`.
- `runner.stop()` joins the worker cleanly.

## Running

```bash
cargo run --manifest-path examples/threaded-capture/Cargo.toml
```

Opens the first available camera, requests the backend's default format
negotiation, and prints ten received-buffer / decoded-image lines, then
stops. The `nokhwa_initialize` call at the start handles macOS camera
permission on first run.

Edit `Cargo.toml` to switch to a specific platform backend
(`input-avfoundation`, `input-v4l`, `input-msmf`) if `input-native`
isn't right for your target. The `runner` feature is required for
`CameraRunner`.
