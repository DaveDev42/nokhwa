# nokhwactl — Capture Example

A command-line tool for testing nokhwa camera backends. It can list devices, inspect camera properties, capture single frames, and stream live video.

## What it demonstrates

- `nokhwa::open(index, OpenRequest)` returns an `OpenedCamera`; the
  example destructures the `Stream` variant for everything except
  `list-devices` (which goes through `query()` directly).
- `list-properties` calls `controls()` and `compatible_formats()` on the
  `StreamCamera` returned by `open`.
- `single` opens with the user's `--requested` format string, calls
  `frame()` once, wraps the `Buffer` into `Frame<Mjpeg>`, and decodes
  via `frame.into_rgb().materialize()` to an `RgbImage`.
- `stream` hands the `OpenedCamera` to `CameraRunner::spawn` and pulls
  `Buffer`s off the runner's bounded channel. With `--display`, the
  `ggez` draw loop wraps each buffer into `Frame::<Mjpeg>::new` and
  calls `.into_rgba().materialize()` for the texture upload. Without
  `--display`, the loop just logs buffer arrival.

## Building

The example depends on `nokhwa` with `input-native` and `runner` features, plus `ggez` for the live display window.

The example opts out of the root Cargo workspace, so use
`--manifest-path` to build it (the package name `nokhwactl` is not
visible to the root workspace's `-p` resolver):

```bash
# From the repository root:
cargo build --manifest-path examples/capture/Cargo.toml
```

If you only need a specific backend, edit `examples/capture/Cargo.toml` and replace `input-native` with the backend you want (e.g. `input-avfoundation`, `input-v4l`, `input-msmf`).

## Usage

### List available cameras

```bash
cargo run --manifest-path examples/capture/Cargo.toml -- list-devices
```

### Inspect camera properties

```bash
# Show all controls and compatible formats for camera 0
cargo run --manifest-path examples/capture/Cargo.toml -- list-properties --device 0 --kind all

# Controls only
cargo run --manifest-path examples/capture/Cargo.toml -- list-properties --device 0 --kind controls

# Compatible formats only
cargo run --manifest-path examples/capture/Cargo.toml -- list-properties --device 0 --kind compatible-formats
```

### Capture a single frame

```bash
# Capture one frame from camera 0 and save it as a PNG
cargo run --manifest-path examples/capture/Cargo.toml -- single --device 0 --save capture.png

# Capture with a specific format request
cargo run --manifest-path examples/capture/Cargo.toml -- single --device 0 --save capture.png \
    --requested "Exact:1920,1080,30,MJPEG"
```

### Stream live video

```bash
# Stream from camera 0 in a window
cargo run --manifest-path examples/capture/Cargo.toml -- stream --device 0 --display

# Stream without display (logs frame sizes to stdout)
cargo run --manifest-path examples/capture/Cargo.toml -- stream --device 0
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
