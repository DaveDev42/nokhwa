# decoder_test

Tests the `nokhwa-core` pixel-format decoder pipeline in isolation — no camera required.

## What it demonstrates

- Constructing a `Buffer` directly from raw bytes + resolution + `FrameFormat`.
- Wrapping it into a `Frame<F>` with the format tag (`Nv12` here).
- Decoding via `frame.into_rgb().materialize()` — the same conversion path used by `Camera<F>::frame_typed()`.

This is the cleanest way to verify the SIMD/scalar decoders for any supported format without needing hardware.

## Running

```bash
cargo run --manifest-path examples/decoder_test/Cargo.toml
```

The example reads `cchlop.nv12` (1920×1080 NV12 raw bytes, included in this directory), decodes it, and writes `cchlop_out_nv12.png`. Compare against the reference `cchlop.png` to validate the decoder.

## Adapting to other formats

Swap the type parameter and `FrameFormat` to test a different decoder:

```rust
use nokhwa_core::buffer::Buffer;
use nokhwa_core::format_types::Yuyv;
use nokhwa_core::frame::{Frame, IntoRgb};
use nokhwa_core::types::{FrameFormat, Resolution};

let buffer = Buffer::new(Resolution::new(w, h), &raw, FrameFormat::YUYV);
let frame: Frame<Yuyv> = Frame::new(buffer);
frame.into_rgb().materialize().unwrap().save("out.png").unwrap();
```
