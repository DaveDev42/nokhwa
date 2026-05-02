# captesting

Minimal single-frame capture example built on the post-0.13
`nokhwa::open` / `OpenedCamera` API.

## What it demonstrates

- `open(CameraIndex::Index(0), OpenRequest::any())` returns an
  `OpenedCamera`; the example destructures the `Stream` variant
  (`OpenedCamera::Stream(mut camera)`) and bails for any other backend
  shape with a clear error.
- `StreamCamera::open()` / `frame()` / `close()` drive a single capture
  cycle.
- The captured `Buffer` is wrapped into the typed
  `Frame<Mjpeg>` and decoded via `frame.into_rgb().materialize()` into
  an `image::ImageBuffer`, which is then written to `turtle.jpeg`.

## Running

```bash
cargo run --manifest-path examples/captesting/Cargo.toml
```

The example opens camera index `0` (edit `src/main.rs` to pick a
different device), captures one MJPEG frame, decodes it, and saves
`turtle.jpeg` to the current directory.

By default the `input-native` feature is enabled. Edit `Cargo.toml` to
pick a specific backend (`input-avfoundation`, `input-v4l`,
`input-msmf`) if needed.
