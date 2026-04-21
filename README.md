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

> **Upgrading from 0.12?** See [`MIGRATING-0.13.md`](./MIGRATING-0.13.md).

Nokhwa 0.13 splits camera capabilities into four traits (`CameraDevice`,
`FrameSource`, `ShutterCapture`, `EventSource`) and dispatches opened devices
through an `OpenedCamera` enum so webcams, DSLRs, and hybrid cameras share one
API surface.

```rust
use nokhwa::{open, OpenRequest, OpenedCamera};
use nokhwa_core::error::NokhwaError;
use nokhwa_core::types::CameraIndex;

fn main() -> Result<(), NokhwaError> {
    let opened = open(CameraIndex::Index(0), OpenRequest::any())?;
    match opened {
        OpenedCamera::Stream(mut cam) => {
            cam.open()?;
            let f = cam.frame()?;
            println!(
                "captured {}x{}",
                f.resolution().width(),
                f.resolution().height()
            );
            cam.close()
        }
        OpenedCamera::Shutter(mut cam) => {
            let photo = cam.capture(std::time::Duration::from_secs(5))?;
            println!("photo: {} bytes", photo.buffer().len());
            Ok(())
        }
        OpenedCamera::Hybrid(mut cam) => {
            cam.open()?;
            let _preview = cam.frame()?;
            let _photo = cam.capture(std::time::Duration::from_secs(5))?;
            Ok(())
        }
    }
}
```

For apps consuming live view, pictures, and events concurrently, use
`CameraRunner` (feature `runner`) which owns the camera on a background
thread and delivers data through `std::sync::mpsc::Receiver` channels.

Runnable examples live in the `examples/` directory
(`stream_camera.rs`, `runner.rs`, plus sub-crate demos).

## API Support

| Backend                          | Input | Query | Query-Device | Platform            |
|----------------------------------|-------|-------|--------------|---------------------|
| Video4Linux (`input-v4l`)        | ✅    | ✅    | ✅           | Linux               |
| MSMF (`input-msmf`)             | ✅    | ✅    | ✅           | Windows             |
| AVFoundation (`input-avfoundation`) | ✅ | ✅    | ✅           | macOS               |
| OpenCV (`input-opencv`)^        | ✅    | ❌    | ❌           | Linux, Windows, Mac |
| GStreamer (`input-gstreamer`)‡  | ❌    | ✅    | ❌           | Linux, Windows, Mac |

✅ Working  ❌ Not Supported

^ = May be bugged. Also supports IP Cameras.

‡ = Session 1 ships device enumeration only (via `DeviceMonitor` filtered to `Video/Source`). Streaming, format negotiation, and controls land in follow-up releases. See `TODO.md`.

## Features

The default feature set enables only `mjpeg`. You **must** additionally enable at least one `input-*` feature.

**Input backends:**
- `input-native`: Auto-selects V4L2 (Linux), MSMF (Windows), or AVFoundation (macOS)
- `input-avfoundation`: macOS/iOS AVFoundation backend
- `input-v4l`: Linux Video4Linux2 backend
- `input-msmf`: Windows Media Foundation backend
- `input-opencv`: Cross-platform OpenCV backend (requires a system OpenCV install). Also the supported path for **IP / RTSP cameras**: pass the URL as `CameraIndex::String` and open with `input-opencv`. The old `NetworkCamera` wrapper was removed in 0.10.0.
- `input-gstreamer`: Cross-platform GStreamer backend via `gstreamer-rs`. Requires a system GStreamer install (`libgstreamer1.0-dev` + `gstreamer1.0-plugins-base` on Ubuntu; upstream installer on macOS / Windows). *Session 1 (current): device enumeration only — `query(ApiBackend::GStreamer)` walks `DeviceMonitor` filtered to `Video/Source` and returns one `CameraInfo` per source.* Opening a camera via this backend is not yet wired up.

**Output features:**
- `output-wgpu`: Copy frames directly into a `wgpu` texture
- `runner`: Enable `CameraRunner` background-thread capture helper

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
