# threaded-capture

Demonstrates background-thread capture via `CallbackCamera<F>` with the 0.12.0 type-safe API.

## What it demonstrates

- `CallbackCamera<Mjpeg>` typed by the format marker — the frame format is carried in the type, so the compiler can verify the decoder you pick matches the camera's output.
- Two ways to consume frames:
  1. **Callback**: the closure passed to `CallbackCamera::new` receives a `Buffer` on every capture. Wrap it into `Frame::<Mjpeg>::new(buffer)` for typed decoding.
  2. **Polling**: `threaded.poll_frame()` returns the latest `Buffer`, also wrappable into `Frame<Mjpeg>`.
- `frame.into_rgba().materialize()` converts to an `RgbaImage`.

## Running

```bash
cargo run --manifest-path examples/threaded-capture/Cargo.toml
```

Opens the first available camera, streams MJPEG at the highest frame rate, and prints frame dimensions from both the callback and the polling loop. `Ctrl+C` to stop.

Edit `Cargo.toml` to switch to a specific platform backend (`input-avfoundation`, `input-v4l`, `input-msmf`) if `input-native` isn't right for your target.
