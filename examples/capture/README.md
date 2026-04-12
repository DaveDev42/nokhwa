# nokhwactl — Capture Example

A command-line tool for testing nokhwa camera backends. It can list devices, inspect camera properties, capture single frames, and stream live video.

## What it demonstrates (0.12.0 API)

- `Camera::open::<Mjpeg>(index, RequestedFormatType::None)` opens a `Camera<Mjpeg>` for the `list-properties` subcommand (no format preference needed, just inspection).
- The `single` subcommand uses `Camera<Mjpeg>::new(index, requested)` because `RequestedCliFormat::make_requested` already builds a `RequestedFormat`.
- `camera.frame_typed()` yields a `Frame<Mjpeg>`, and `frame.into_rgb().materialize()` decodes to an `RgbImage`.
- `CallbackCamera<Mjpeg>` drives the `stream` subcommand. With `--display`, the `ggez` draw loop pulls each forwarded `Buffer` out of an mpsc channel, wraps it into `Frame::<Mjpeg>::new(buffer)`, and calls `.into_rgba().materialize()` for the texture upload. Without `--display`, the callback only logs the buffer size.

## Building

The example depends on `nokhwa` with `input-native` and `output-threaded` features, plus `ggez` for the live display window.

```bash
# From the repository root:
cargo build -p nokhwactl
```

If you only need a specific backend, edit `examples/capture/Cargo.toml` and replace `input-native` with the backend you want (e.g. `input-avfoundation`, `input-v4l`, `input-msmf`).

## Usage

### List available cameras

```bash
cargo run -p nokhwactl -- list-devices
```

### Inspect camera properties

```bash
# Show all controls and compatible formats for camera 0
cargo run -p nokhwactl -- list-properties --device 0 --kind all

# Controls only
cargo run -p nokhwactl -- list-properties --device 0 --kind controls

# Compatible formats only
cargo run -p nokhwactl -- list-properties --device 0 --kind compatible-formats
```

### Capture a single frame

```bash
# Capture one frame from camera 0 and save it as a PNG
cargo run -p nokhwactl -- single --device 0 --save capture.png

# Capture with a specific format request
cargo run -p nokhwactl -- single --device 0 --save capture.png \
    --requested "Exact:1920,1080,30,MJPEG"
```

### Stream live video

```bash
# Stream from camera 0 in a window
cargo run -p nokhwactl -- stream --device 0 --display

# Stream without display (logs frame sizes to stdout)
cargo run -p nokhwactl -- stream --device 0
```

### Format request strings

The `--requested` flag accepts a string in the format `"<Type>:<options>"`:

| Request type                 | Syntax                            | Example                               |
|------------------------------|-----------------------------------|---------------------------------------|
| Absolute highest resolution  | `AbsoluteHighestResolution`       | `AbsoluteHighestResolution`           |
| Absolute highest frame rate  | `AbsoluteHighestFrameRate`        | `AbsoluteHighestFrameRate`            |
| Highest resolution at size   | `HighestResolution:<w>,<h>`       | `HighestResolution:1920,1080`         |
| Highest FPS at rate          | `HighestFrameRate:<fps>`          | `HighestFrameRate:60`                 |
| Exact format                 | `Exact:<w>,<h>,<fps>,<fourcc>`    | `Exact:1280,720,30,MJPEG`            |
| Closest match                | `Closest:<w>,<h>,<fps>,<fourcc>`  | `Closest:1920,1080,60,MJPEG`         |
| Any available format         | `None`                            | `None`                                |

## Platform notes

- **macOS**: The first run may trigger a camera permission dialog. The example calls `nokhwa_initialize()` which handles this.
- **Linux**: Ensure your user has access to `/dev/video*` devices (usually via the `video` group).
- **Windows**: Media Foundation is used by default. No special setup required.
