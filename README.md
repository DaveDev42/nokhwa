[![CI](https://github.com/DaveDev42/nokhwa/actions/workflows/lint.yml/badge.svg)](https://github.com/DaveDev42/nokhwa/actions/workflows/lint.yml) [![Build](https://github.com/DaveDev42/nokhwa/actions/workflows/build-matrix.yml/badge.svg)](https://github.com/DaveDev42/nokhwa/actions/workflows/build-matrix.yml) [![docs.rs](https://img.shields.io/docsrs/nokhwa)](https://docs.rs/nokhwa/latest/nokhwa/)
# nokhwa
Nokhwa(녹화): Korean word meaning "to record".

A Simple-to-use, cross-platform Rust Webcam Capture Library

> **Note:** This is a maintained fork of [l1npengtul/nokhwa](https://github.com/l1npengtul/nokhwa) with modernized macOS bindings (objc2), performance improvements, and CI infrastructure.

## Using nokhwa

Since this fork is not published to crates.io, add it as a git dependency:
```toml
[dependencies.nokhwa]
git = "https://github.com/DaveDev42/nokhwa.git"
branch = "main"
# Use the native input backend for your platform
features = ["input-native"]
```

To use a specific backend instead of `input-native`:
```toml
features = ["input-avfoundation"]  # macOS
features = ["input-v4l"]           # Linux
features = ["input-msmf"]          # Windows
```

## Quick Start

Nokhwa 0.12 uses a **type-safe frame API**: `Camera<F>` is parameterized by a
`CaptureFormat` marker (e.g. `Yuyv`, `Mjpeg`, `Nv12`, `Gray`, `RawRgb`,
`RawBgr`), and `Frame<F>` carries that format tag at compile time. Invalid
conversions (e.g. a grayscale frame into RGB) are caught by the compiler.

```rust
use nokhwa::Camera;
use nokhwa::utils::{CameraIndex, RequestedFormatType};
use nokhwa_core::format_types::Yuyv;
use nokhwa_core::frame::IntoRgb;

fn main() -> Result<(), nokhwa::NokhwaError> {
    // Open the first camera capturing YUYV at the highest available frame rate.
    let mut camera = Camera::open::<Yuyv>(
        CameraIndex::Index(0),
        RequestedFormatType::AbsoluteHighestFrameRate,
    )?;

    // Start the stream and grab a typed frame.
    camera.open_stream()?;
    let frame = camera.frame_typed()?;          // Frame<Yuyv>
    println!("captured {}x{}", frame.resolution().width(), frame.resolution().height());

    // Decode lazily, then materialize into an `image::RgbImage`.
    let rgb = frame.into_rgb().materialize()?;  // ImageBuffer<Rgb<u8>, Vec<u8>>
    println!("decoded {} bytes", rgb.len());
    Ok(())
}
```

Other common conversions: `frame.into_rgba().materialize()` →
`ImageBuffer<Rgba<u8>>`, `frame.into_luma().materialize()` →
`ImageBuffer<Luma<u8>>`.

### Compile-time format safety

`Frame<Gray>` does **not** implement `IntoRgb` — grayscale carries no color
information, so converting to RGB is a category error and a compile-time
failure:

```rust,compile_fail
use nokhwa_core::format_types::Gray;
use nokhwa_core::frame::{Frame, IntoRgb};

fn demo(frame: Frame<Gray>) {
    // error[E0277]: the trait bound `Frame<Gray>: IntoRgb` is not satisfied
    let _ = frame.into_rgb();
}
```

Use `frame.into_luma().materialize()` instead for `Gray` frames.

A command line app made with `nokhwa` can be found in the `examples` folder.

## API Support

| Backend                          | Input | Query | Query-Device | Platform            |
|----------------------------------|-------|-------|--------------|---------------------|
| Video4Linux (`input-v4l`)        | ✅    | ✅    | ✅           | Linux               |
| MSMF (`input-msmf`)             | ✅    | ✅    | ✅           | Windows             |
| AVFoundation (`input-avfoundation`) | ✅ | ✅    | ✅           | macOS               |
| OpenCV (`input-opencv`)^        | ✅    | ❌    | ❌           | Linux, Windows, Mac |

✅ Working  ❌ Not Supported

^ = May be bugged. Also supports IP Cameras.

## Features

The crate's default features enable only `mjpeg`. You **must** additionally enable at least one `input-*` feature.

**Input backends:**
- `input-native`: Auto-selects V4L2 (Linux), MSMF (Windows), or AVFoundation (macOS)
- `input-avfoundation`: macOS/iOS AVFoundation backend
- `input-v4l`: Linux Video4Linux2 backend
- `input-msmf`: Windows Media Foundation backend
- `input-opencv`: Cross-platform OpenCV backend

**Output features:**
- `output-wgpu`: Copy frames directly into a `wgpu` texture
- `output-threaded`: Enable `CallbackCamera` with background capture thread

**Other features:**
- `mjpeg`: MJPEG decoding via `mozjpeg` (enabled by default)
- `serialize`: `serde` support for core types

## Minimum Supported Rust Version

Regular builds work on **stable Rust**. The `docs-features` feature requires **nightly** (`#![feature(doc_cfg)]`). Development environment is configured via `flake.nix` (nightly).

## Issues
If you are making an issue, please make sure that
 - It has not been made yet
 - Attach what you were doing, your environment, steps to reproduce, and backtrace.

## Contributing
Contributions are welcome!
 - Please `rustfmt` all your code and adhere to the clippy lints (unless necessary not to do so)
 - Please limit use of `unsafe`
 - All contributions are under the Apache 2.0 license unless otherwise specified

## Sponsors

This project is a fork of [l1npengtul/nokhwa](https://github.com/l1npengtul/nokhwa). The following sponsors supported the original author's work on nokhwa:

- $40/mo sponsors:
  - [erlend-sh](https://github.com/erlend-sh)
  - [DanielMSchmidt](https://github.com/DanielMSchmidt)
- $5/mo sponsors:
  - [remifluff](https://github.com/remifluff)
  - [gennyble](https://github.com/gennyble)

Please consider [sponsoring the original author](https://github.com/sponsors/l1npengtul) to support the continued development of nokhwa.
