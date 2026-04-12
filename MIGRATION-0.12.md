# Migration guide: nokhwa 0.11 → 0.12

Nokhwa 0.12 replaces the runtime `FormatDecoder` trait with a **type-safe
frame API**. `Camera` and `Frame` are now generic over a
[`CaptureFormat`](https://docs.rs/nokhwa-core/latest/nokhwa_core/format_types/trait.CaptureFormat.html)
marker (`Yuyv`, `Mjpeg`, `Nv12`, `Gray`, `RawRgb`, `RawBgr`), so the compiler
can reject invalid operations (e.g. converting a grayscale frame to RGB)
instead of erroring at runtime.

## API map

| 0.11                                                  | 0.12                                                        |
|-------------------------------------------------------|-------------------------------------------------------------|
| `Camera::new(index, RequestedFormat::new::<RgbFormat>(…))` | `Camera::open::<F>(index, RequestedFormatType::…)`      |
| `camera.frame()` → `Buffer`                           | `camera.frame_typed()` → `Frame<F>`                         |
| `buffer.decode_image::<RgbFormat>()`                  | `frame.into_rgb().materialize()`                            |
| `buffer.decode_image::<RgbAFormat>()`                 | `frame.into_rgba().materialize()`                           |
| `buffer.decode_image::<LumaFormat>()`                 | `frame.into_luma().materialize()`                           |
| `FormatDecoder` trait                                 | `CaptureFormat` marker + `IntoRgb`/`IntoRgba`/`IntoLuma`    |
| `RgbFormat` / `RgbAFormat` / `LumaFormat` ZSTs        | Produced by `into_rgb()` / `into_rgba()` / `into_luma()`    |
| `RequestedFormat<F>` (generic over output)            | `RequestedFormatType` + wire format picked by `Camera::open::<F>` (`RequestedFormat` still exists for advanced use) |
| `decoding` feature flag                               | `mjpeg` feature flag                                        |
| `image` pulled in via default `decoding` feature      | `image` is an unconditional dependency                      |

## Before and after

### 0.11 — runtime-dispatched decode

```rust,ignore
use nokhwa::{Camera, pixel_format::RgbFormat};
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};

let index = CameraIndex::Index(0);
let requested = RequestedFormat::new::<RgbFormat>(
    RequestedFormatType::AbsoluteHighestFrameRate,
);
let mut camera = Camera::new(index, requested)?;

let frame = camera.frame()?;                         // Buffer
let decoded = frame.decode_image::<RgbFormat>()?;    // runtime format check
```

### 0.12 — compile-time-typed decode

```rust,ignore
use nokhwa::Camera;
use nokhwa::utils::{CameraIndex, RequestedFormatType};
use nokhwa_core::format_types::Yuyv;
use nokhwa_core::frame::IntoRgb;

let mut camera = Camera::open::<Yuyv>(
    CameraIndex::Index(0),
    RequestedFormatType::AbsoluteHighestFrameRate,
)?;
camera.open_stream()?;

let frame = camera.frame_typed()?;            // Frame<Yuyv>
let decoded = frame.into_rgb().materialize()?; // ImageBuffer<Rgb<u8>>
```

## Removed items

- **`FormatDecoder` trait** — replaced by the `CaptureFormat` marker trait
  plus the `IntoRgb` / `IntoRgba` / `IntoLuma` conversion traits.
- **`RgbFormat`, `RgbAFormat`, `LumaFormat` ZSTs** — the output pixel layout
  is now chosen by which conversion method you call on a `Frame<F>`.
- **`Buffer::decode_image::<F>()`** — use `Frame::into_rgb()` (or `into_rgba`
  / `into_luma`) followed by `materialize()`.
- **`RequestedFormat<F>`** — the output-format type parameter is gone; the
  wire format is expressed as `Camera::open::<F>(..)`. Pass a
  `RequestedFormatType` directly for the resolution/framerate strategy.
- **`decoding` feature flag** — renamed to **`mjpeg`** (scope is specifically
  MJPEG decoding via `mozjpeg`).

## New requirements

- **`image` is now an unconditional dependency.** In 0.11 it was pulled in by
  the default `decoding` feature; in 0.12 it is always linked because
  `Frame::into_*()` materializes to `image::ImageBuffer`. If you previously
  disabled default features to drop `image`, that path is gone.
- **Streaming must be opened explicitly.** Call `camera.open_stream()` before
  `camera.frame_typed()`. Some 0.11 backends implicitly opened the stream on
  first `frame()`; 0.12 is consistent in requiring `open_stream()` first.

## Compile-time safety you now get for free

`Frame<Gray>` does not implement `IntoRgb` or `IntoRgba` — grayscale has no
color information, so converting to RGB is rejected by the compiler:

```rust,compile_fail
use nokhwa_core::format_types::Gray;
use nokhwa_core::frame::{Frame, IntoRgb};

fn demo(frame: Frame<Gray>) {
    let _ = frame.into_rgb();   // error[E0277]: `Frame<Gray>: IntoRgb` is not satisfied
}
```

Use `frame.into_luma().materialize()` for grayscale frames.

## Picking a `CaptureFormat`

| Format    | Typical source                              | Best conversion     |
|-----------|---------------------------------------------|---------------------|
| `Yuyv`    | USB UVC webcams (uncompressed)              | `into_rgb` / `into_luma` |
| `Nv12`    | Many integrated and Windows cameras         | `into_rgb` / `into_luma` |
| `Mjpeg`   | High-resolution USB webcams (compressed)    | `into_rgb` (via mozjpeg)  |
| `Gray`    | Monochrome / IR cameras                     | `into_luma` only    |
| `RawRgb`  | Cameras exposing packed RGB888              | `into_rgb`          |
| `RawBgr`  | Some industrial / screen-capture sources    | `into_rgb`          |

If you don't know which format a camera supports, enumerate devices with
`nokhwa::query()` then open one and call `Camera::compatible_fourcc()` or
`Camera::compatible_camera_formats()` to discover supported formats
(platform-dependent).
