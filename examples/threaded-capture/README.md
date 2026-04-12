# threaded-capture

Demonstrates background-thread capture via `CallbackCamera<F>` with the 0.12.0 type-safe API.

## What it demonstrates

- `CallbackCamera<Mjpeg>` typed by the format marker — the frame format is carried in the type, so the compiler can verify the decoder you pick matches the camera's output.
- The callback (installed via `CallbackCamera::new`) fires on every captured `Buffer`; here it just logs buffer arrival so you can see it runs on the background thread.
- The main thread uses `threaded.poll_frame()` to pull the latest `Buffer`, wraps it into `Frame::<Mjpeg>::new(buffer)`, and decodes via `frame.into_rgba().materialize()`.

## Running

```bash
cargo run --manifest-path examples/threaded-capture/Cargo.toml
```

Opens the first available camera, requests MJPEG at the highest available frame rate, and prints interleaved `callback:` (background thread) and `poll:` (main thread) lines. `Ctrl+C` to stop.

Edit `Cargo.toml` to switch to a specific platform backend (`input-avfoundation`, `input-v4l`, `input-msmf`) if `input-native` isn't right for your target.
