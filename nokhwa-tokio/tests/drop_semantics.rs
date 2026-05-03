// Integration tests for `nokhwa-tokio`. Originally a single link-check;
// expanded to drive `TokioCameraRunner::spawn` end-to-end against a
// cross-platform fake camera built on `nokhwa_core::testing` and the
// `nokhwa_backend!` macro. The earlier "needs a fake camera that does
// not exist yet" comment is now stale — `MockFrameSource` is exactly
// that fake.

use std::borrow::Cow;
use std::time::Duration;

use nokhwa::nokhwa_backend;
use nokhwa::{OpenedCamera, RunnerConfig};
use nokhwa_core::buffer::Buffer;
use nokhwa_core::error::NokhwaError;
use nokhwa_core::testing::{mock_frame, MockFrameSource};
use nokhwa_core::traits::{CameraDevice, FrameSource};
use nokhwa_core::types::{
    ApiBackend, CameraControl, CameraFormat, CameraInfo, ControlValueSetter, FrameFormat,
    KnownCameraControl,
};
use nokhwa_tokio::TokioCameraRunner;

// Local newtype so the orphan rule lets us invoke `nokhwa_backend!` on
// `MockFrameSource` from this integration-test crate. Identical shape
// to the `FrameOnly` newtype used in `nokhwa/tests/session.rs`.
struct FrameOnly(MockFrameSource);

impl CameraDevice for FrameOnly {
    fn backend(&self) -> ApiBackend {
        self.0.backend()
    }
    fn info(&self) -> &CameraInfo {
        self.0.info()
    }
    fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        self.0.controls()
    }
    fn set_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.0.set_control(id, value)
    }
}

impl FrameSource for FrameOnly {
    fn negotiated_format(&self) -> CameraFormat {
        self.0.negotiated_format()
    }
    fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
        self.0.set_format(f)
    }
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        self.0.compatible_formats()
    }
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        self.0.compatible_fourcc()
    }
    fn open(&mut self) -> Result<(), NokhwaError> {
        self.0.open()
    }
    fn is_open(&self) -> bool {
        self.0.is_open()
    }
    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.0.frame()
    }
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        self.0.frame_raw()
    }
    fn close(&mut self) -> Result<(), NokhwaError> {
        self.0.close()
    }
}

nokhwa_backend!(FrameOnly: FrameSource);

fn make_stream_camera_with_frames(count: usize) -> OpenedCamera {
    let mut src = MockFrameSource::new(0);
    for _ in 0..count {
        src.push_frame(mock_frame(4, 4, FrameFormat::YUYV));
    }
    OpenedCamera::from_device(Box::new(FrameOnly(src)))
}

#[tokio::test(flavor = "current_thread")]
async fn library_links_under_tokio_runtime() {
    let _ = tokio::task::spawn_blocking(|| 1 + 1).await.unwrap();
}

/// `TokioCameraRunner::spawn` on a stream-only fake camera must wire
/// the frames forwarder so `frames_mut().recv().await` yields buffers.
/// `pictures_mut()` and `events_mut()` must return `None` — the fake
/// has neither shutter nor event source. Pin the per-channel
/// `Option<&mut Receiver<…>>` shape so a regression in
/// `TokioCameraRunner::spawn` (e.g. always wiring all three forwarders)
/// fails fast instead of silently leaking tasks.
#[tokio::test(flavor = "current_thread")]
async fn tokio_runner_stream_only_yields_frames_no_pictures_no_events() {
    let opened = make_stream_camera_with_frames(3);
    let mut runner = TokioCameraRunner::spawn(opened, RunnerConfig::default())
        .expect("TokioCameraRunner::spawn on stream-only camera");

    assert!(
        runner.pictures_mut().is_none(),
        "stream-only camera must not have a pictures channel"
    );
    assert!(
        runner.events_mut().is_none(),
        "stream-only camera (no EventSource) must not have an events channel"
    );

    let frames = runner
        .frames_mut()
        .expect("stream-only camera must have a frames channel");
    let buf = tokio::time::timeout(Duration::from_secs(1), frames.recv())
        .await
        .expect("recv timed out — forwarder did not bridge sync→async")
        .expect("forwarder closed before delivering any frame");
    assert_eq!(
        buf.source_frame_format(),
        FrameFormat::YUYV,
        "frame round-tripped through the bridge unchanged"
    );

    runner.stop().await.expect("stop on running runner");
}

/// `TokioCameraRunner::stop().await` is the documented graceful
/// shutdown path: drains forwarders, drops async receivers so the
/// sync runner sees them disconnect, then `spawn_blocking`s the inner
/// `CameraRunner` drop. Pin that calling `stop` returns `Ok(())` even
/// when there were no in-flight frames waiting (i.e. the runner has
/// nothing buffered to flush). A regression that, e.g., panicked when
/// `inner` was already taken would surface here.
#[tokio::test(flavor = "current_thread")]
async fn tokio_runner_stop_succeeds_on_idle_runner() {
    let opened = make_stream_camera_with_frames(0);
    let runner = TokioCameraRunner::spawn(opened, RunnerConfig::default())
        .expect("TokioCameraRunner::spawn on idle camera");
    runner
        .stop()
        .await
        .expect("stop must succeed on an idle runner with no buffered frames");
}

/// After `stop().await`, the runner is consumed (no further calls
/// possible) — but `Drop` runs on the way out and must be a no-op
/// because `inner` was already taken. Pin that the drop path doesn't
/// panic when `inner` is `None`. We verify the contract indirectly by
/// stopping then letting the binding fall out of scope; a regression
/// in `Drop` (e.g. unwrapping `inner` instead of `take`-ing) would
/// surface as a panic from the test runtime.
#[tokio::test(flavor = "current_thread")]
async fn tokio_runner_drop_after_stop_is_noop() {
    let opened = make_stream_camera_with_frames(0);
    let runner = TokioCameraRunner::spawn(opened, RunnerConfig::default())
        .expect("TokioCameraRunner::spawn");
    runner.stop().await.expect("stop");
    // Falls out of scope here; Drop must not panic on already-stopped state.
}

/// Drop without explicit stop must not panic and must not deadlock —
/// the `Drop` impl detects the current tokio runtime via
/// `Handle::try_current()` and queues the inner-runner shutdown via
/// `spawn_blocking`. Pin that synchronous test bodies don't hang or
/// panic on this path. (We don't `await` the queued blocking task
/// here — `Drop` is fire-and-forget by design — but this test
/// catches a regression where, e.g., `try_current()` returned `Err`
/// inside a runtime and triggered the synchronous-drop branch.)
#[tokio::test(flavor = "current_thread")]
async fn tokio_runner_drop_without_stop_inside_runtime_does_not_panic() {
    let opened = make_stream_camera_with_frames(0);
    let runner = TokioCameraRunner::spawn(opened, RunnerConfig::default())
        .expect("TokioCameraRunner::spawn");
    drop(runner);
    // Yield once so any spawn_blocking task queued by Drop has a
    // chance to start; if Drop deadlocked, the executor would hang
    // here instead of returning control.
    tokio::task::yield_now().await;
}

/// `trigger()` and `set_control()` on a running stream-only runner
/// are documented no-ops (the worker thread accepts the commands but
/// does not drive a shutter or apply controls on a pure
/// `MockFrameSource`). Pin that both surface `Ok` on the happy path
/// — a regression that returned `Err` (or panicked) on a non-shutter
/// backend would silently break async clients that call these as
/// fire-and-forget signals.
#[tokio::test(flavor = "current_thread")]
async fn tokio_runner_trigger_and_set_control_no_op_on_stream() {
    let opened = make_stream_camera_with_frames(0);
    let runner = TokioCameraRunner::spawn(opened, RunnerConfig::default())
        .expect("TokioCameraRunner::spawn");

    runner
        .trigger()
        .expect("trigger on running stream runner is a no-op Ok");
    runner
        .set_control(
            KnownCameraControl::Brightness,
            ControlValueSetter::Integer(0),
        )
        .expect("set_control on running stream runner is a no-op Ok");

    runner.stop().await.expect("stop");
}
