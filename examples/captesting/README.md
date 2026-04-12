# captesting

Minimal single-frame capture example that showcases the 0.12.0 type-safe API.

## What it demonstrates

- `Camera::open::<Mjpeg>(index, RequestedFormatType::AbsoluteHighestResolution)` returns a `Camera<Mjpeg>` whose frame format is known at compile time.
- `camera.frame_typed()` returns a `Frame<Mjpeg>` — no manual `Frame::new(buffer)` wrapping required.
- `frame.into_rgb().materialize()` decodes directly into an `image::ImageBuffer`.

## Running

```bash
cargo run --manifest-path examples/captesting/Cargo.toml
```

The example opens camera index `0` (edit `src/main.rs` to pick a different device), captures one MJPEG frame, decodes it, and saves `turtle.jpeg` to the current directory.

By default the `input-native` feature is enabled. Edit `Cargo.toml` to pick a specific backend (`input-avfoundation`, `input-v4l`, `input-msmf`) if needed.
