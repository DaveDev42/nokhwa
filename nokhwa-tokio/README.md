# nokhwa-tokio

Tokio integration for [`nokhwa`](https://crates.io/crates/nokhwa).

Wraps the sync `CameraRunner` with `TokioCameraRunner`, which exposes
`tokio::sync::mpsc::Receiver`s you can `.recv().await`. Drop is
async-safe: the underlying worker is joined on a `spawn_blocking` task
when dropped inside a tokio runtime, so the async executor is not
blocked.

## Example

```rust,no_run
use nokhwa::{CameraSession, OpenRequest, RunnerConfig};
use nokhwa_core::types::CameraIndex;
use nokhwa_tokio::TokioCameraRunner;
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), nokhwa_core::error::NokhwaError> {
    let opened = CameraSession::open(CameraIndex::Index(0), OpenRequest::any())?;
    let mut runner = TokioCameraRunner::spawn(opened, RunnerConfig::default())?;
    if let Some(rx) = runner.frames_mut() {
        if let Some(buf) = rx.recv().await {
            println!("frame: {} bytes", buf.buffer().len());
        }
    }
    runner.stop().await
}
```

See also `examples/tokio_runner.rs` in this crate.

## Tokio features

This crate depends on `tokio` with only `sync` and `rt` — the minimum
needed for `mpsc` and `spawn_blocking`. Your application will typically
pull in more tokio features itself.

## Targets

The crate auto-selects the native `nokhwa` input backend per OS
(AVFoundation on macOS, V4L on Linux, Media Foundation on Windows). On
other targets, add a suitable input backend on `nokhwa` directly.

## License

Apache-2.0, same as `nokhwa`.
