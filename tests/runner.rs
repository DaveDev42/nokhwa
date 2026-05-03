/*
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#![cfg(feature = "runner")]

//! Integration tests for [`nokhwa::runner`].

use std::borrow::Cow;
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;

use nokhwa::nokhwa_backend;
use nokhwa::{CameraRunner, OpenedCamera, Overflow, RunnerConfig};
use nokhwa_core::buffer::Buffer;
use nokhwa_core::error::NokhwaError;
use nokhwa_core::testing::{mock_frame, MockFrameSource, MockHybrid, MockShutter, MpscEventPoll};
use nokhwa_core::traits::{
    CameraDevice, CameraEvent, EventPoll, EventSource, FrameSource, ShutterCapture,
};
use nokhwa_core::types::{
    ApiBackend, CameraControl, CameraFormat, CameraInfo, ControlValueSetter, FrameFormat,
    KnownCameraControl,
};

// ─────────────── local newtype wrappers (orphan-rule shim) ────────────

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

struct ShutterOnly(MockShutter);

impl CameraDevice for ShutterOnly {
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
impl ShutterCapture for ShutterOnly {
    fn trigger(&mut self) -> Result<(), NokhwaError> {
        self.0.trigger()
    }
    fn take_picture(&mut self, t: Duration) -> Result<Buffer, NokhwaError> {
        self.0.take_picture(t)
    }
}

/// Hybrid with events — MockHybrid + an mpsc event sender the test holds.
struct HybridWithEvents {
    inner: MockHybrid,
    poll: Option<Box<dyn EventPoll + Send>>,
}

impl HybridWithEvents {
    fn new(inner: MockHybrid, poll: Box<dyn EventPoll + Send>) -> Self {
        Self {
            inner,
            poll: Some(poll),
        }
    }
}

impl CameraDevice for HybridWithEvents {
    fn backend(&self) -> ApiBackend {
        self.inner.backend()
    }
    fn info(&self) -> &CameraInfo {
        self.inner.info()
    }
    fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        self.inner.controls()
    }
    fn set_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.inner.set_control(id, value)
    }
}
impl FrameSource for HybridWithEvents {
    fn negotiated_format(&self) -> CameraFormat {
        self.inner.negotiated_format()
    }
    fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
        self.inner.set_format(f)
    }
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        self.inner.compatible_formats()
    }
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        self.inner.compatible_fourcc()
    }
    fn open(&mut self) -> Result<(), NokhwaError> {
        self.inner.open()
    }
    fn is_open(&self) -> bool {
        self.inner.is_open()
    }
    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.inner.frame()
    }
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        self.inner.frame_raw()
    }
    fn close(&mut self) -> Result<(), NokhwaError> {
        self.inner.close()
    }
}
impl ShutterCapture for HybridWithEvents {
    fn trigger(&mut self) -> Result<(), NokhwaError> {
        self.inner.trigger()
    }
    fn take_picture(&mut self, t: Duration) -> Result<Buffer, NokhwaError> {
        self.inner.take_picture(t)
    }
}
impl EventSource for HybridWithEvents {
    fn take_events(&mut self) -> Result<Box<dyn EventPoll + Send>, NokhwaError> {
        self.poll
            .take()
            .ok_or(NokhwaError::UnsupportedOperationError(ApiBackend::Browser))
    }
}

nokhwa_backend!(FrameOnly: FrameSource);
nokhwa_backend!(ShutterOnly: ShutterCapture);
nokhwa_backend!(HybridWithEvents: FrameSource, ShutterCapture, EventSource);

fn make_frame_only() -> FrameOnly {
    let mut s = MockFrameSource::new(0);
    // Push a handful of frames so the runner has something to deliver.
    for _ in 0..8 {
        s.push_frame(mock_frame(4, 4, FrameFormat::YUYV));
    }
    FrameOnly(s)
}

fn make_shutter_only() -> ShutterOnly {
    ShutterOnly(MockShutter::new(vec![
        mock_frame(4, 4, FrameFormat::MJPEG),
        mock_frame(4, 4, FrameFormat::MJPEG),
    ]))
}

fn make_hybrid_with_events() -> (HybridWithEvents, Sender<CameraEvent>) {
    let mut h = MockHybrid::new(0, vec![mock_frame(4, 4, FrameFormat::MJPEG)]);
    for _ in 0..8 {
        h.push_frame(mock_frame(8, 8, FrameFormat::YUYV));
    }
    let (tx, rx) = channel();
    let poll: Box<dyn EventPoll + Send> = Box::new(MpscEventPoll::new(rx));
    (HybridWithEvents::new(h, poll), tx)
}

#[test]
fn runner_stream_delivers_frames() {
    let opened = OpenedCamera::from_device(Box::new(make_frame_only()));
    let runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    let rx = runner.frames().expect("stream runner has frames channel");
    let _buf = rx
        .recv_timeout(Duration::from_millis(500))
        .expect("frame timed out");
    assert!(runner.pictures().is_none());
}

#[test]
fn runner_shutter_delivers_pictures_on_trigger() {
    let opened = OpenedCamera::from_device(Box::new(make_shutter_only()));
    let runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    runner.trigger().unwrap();
    let rx = runner
        .pictures()
        .expect("shutter runner has pictures channel");
    let _buf = rx
        .recv_timeout(Duration::from_millis(500))
        .expect("picture timed out");
    assert!(runner.frames().is_none());
}

#[test]
fn runner_hybrid_delivers_both_and_events() {
    let (hybrid, tx) = make_hybrid_with_events();
    tx.send(CameraEvent::Disconnected).unwrap();
    let opened = OpenedCamera::from_device(Box::new(hybrid));
    let runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    let _ = runner
        .frames()
        .unwrap()
        .recv_timeout(Duration::from_millis(500))
        .unwrap();
    runner.trigger().unwrap();
    let _ = runner
        .pictures()
        .unwrap()
        .recv_timeout(Duration::from_millis(500))
        .unwrap();
    let _ = runner
        .events()
        .unwrap()
        .recv_timeout(Duration::from_millis(500))
        .unwrap();
}

#[test]
fn runner_drop_joins_thread() {
    let opened = OpenedCamera::from_device(Box::new(make_frame_only()));
    let runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    drop(runner);
}

/// M10: verify `RunnerConfig::shutter_timeout` is actually forwarded to
/// `ShutterCapture::take_picture`. An instrumented shutter records the
/// `timeout` passed to it and the runner is spawned with a custom value.
#[test]
fn runner_shutter_timeout_is_forwarded() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    struct TimeoutProbe {
        info: CameraInfo,
        observed_ms: Arc<AtomicU64>,
    }

    impl CameraDevice for TimeoutProbe {
        fn backend(&self) -> ApiBackend {
            ApiBackend::Browser
        }
        fn info(&self) -> &CameraInfo {
            &self.info
        }
        fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            Ok(vec![])
        }
        fn set_control(
            &mut self,
            _id: KnownCameraControl,
            _v: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            Ok(())
        }
    }

    impl ShutterCapture for TimeoutProbe {
        fn trigger(&mut self) -> Result<(), NokhwaError> {
            Ok(())
        }
        fn take_picture(&mut self, timeout: Duration) -> Result<Buffer, NokhwaError> {
            self.observed_ms
                .store(timeout.as_millis() as u64, Ordering::SeqCst);
            Ok(mock_frame(4, 4, FrameFormat::MJPEG))
        }
    }

    nokhwa_backend!(TimeoutProbe: ShutterCapture);

    let observed = Arc::new(AtomicU64::new(0));
    let probe = TimeoutProbe {
        info: CameraInfo::new(
            "probe",
            "probe",
            "probe",
            nokhwa_core::types::CameraIndex::Index(0),
        ),
        observed_ms: Arc::clone(&observed),
    };
    let opened = OpenedCamera::from_device(Box::new(probe));
    let cfg = RunnerConfig {
        shutter_timeout: Duration::from_millis(1234),
        ..RunnerConfig::default()
    };
    let runner = CameraRunner::spawn(opened, cfg).unwrap();
    runner.trigger().unwrap();
    let _ = runner
        .pictures()
        .expect("shutter runner has pictures channel")
        .recv_timeout(Duration::from_secs(1))
        .expect("picture timed out");
    assert_eq!(observed.load(Ordering::SeqCst), 1234);
}

// ──────────────────── public-API default-value pins ───────────────────

#[test]
fn overflow_default_is_drop_newest() {
    assert_eq!(Overflow::default(), Overflow::DropNewest);
}

#[test]
fn overflow_derives_copy_eq() {
    let a = Overflow::DropOldest;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(Overflow::DropNewest, Overflow::Block);
}

#[test]
fn runner_config_default_pins_field_values() {
    let cfg = RunnerConfig::default();
    assert_eq!(cfg.poll_interval, Duration::from_millis(10));
    assert_eq!(cfg.event_tick, Duration::from_millis(50));
    assert_eq!(cfg.shutter_timeout, Duration::from_secs(5));
    assert_eq!(cfg.frames_capacity, 4);
    assert_eq!(cfg.pictures_capacity, 8);
    assert_eq!(cfg.events_capacity, 32);
    assert_eq!(cfg.overflow, Overflow::DropNewest);
}

#[test]
fn runner_config_is_copy() {
    let cfg = RunnerConfig::default();
    let copied = cfg;
    assert_eq!(cfg.frames_capacity, copied.frames_capacity);
}

// ──────────────────── stop() and take_*() coverage ────────────────────

#[test]
fn runner_stop_returns_ok_on_stream_backend() {
    let opened = OpenedCamera::from_device(Box::new(make_frame_only()));
    let runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    runner.stop().unwrap();
}

#[test]
fn runner_stop_returns_ok_on_shutter_backend() {
    let opened = OpenedCamera::from_device(Box::new(make_shutter_only()));
    let runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    runner.stop().unwrap();
}

#[test]
fn runner_stop_returns_ok_on_hybrid_backend() {
    let (hybrid, _tx) = make_hybrid_with_events();
    let opened = OpenedCamera::from_device(Box::new(hybrid));
    let runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    runner.stop().unwrap();
}

#[test]
fn runner_take_frames_idempotent_on_stream() {
    let opened = OpenedCamera::from_device(Box::new(make_frame_only()));
    let mut runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    let rx = runner
        .take_frames()
        .expect("first take should yield receiver");
    assert!(runner.take_frames().is_none(), "second take must be None");
    assert!(
        runner.frames().is_none(),
        "frames() must be None after take_frames()"
    );
    let _buf = rx
        .recv_timeout(Duration::from_millis(500))
        .expect("owned receiver still delivers frames");
}

#[test]
fn runner_take_pictures_idempotent_on_shutter() {
    let opened = OpenedCamera::from_device(Box::new(make_shutter_only()));
    let mut runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    let rx = runner
        .take_pictures()
        .expect("first take should yield receiver");
    assert!(runner.take_pictures().is_none(), "second take must be None");
    assert!(
        runner.pictures().is_none(),
        "pictures() must be None after take_pictures()"
    );
    runner.trigger().unwrap();
    let _buf = rx
        .recv_timeout(Duration::from_millis(500))
        .expect("owned receiver still delivers pictures after trigger");
}

#[test]
fn runner_take_pictures_none_on_stream_backend() {
    let opened = OpenedCamera::from_device(Box::new(make_frame_only()));
    let mut runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    assert!(runner.take_pictures().is_none());
    assert!(runner.take_events().is_none());
}

#[test]
fn runner_take_frames_none_on_shutter_backend() {
    let opened = OpenedCamera::from_device(Box::new(make_shutter_only()));
    let mut runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    assert!(runner.take_frames().is_none());
    assert!(runner.take_events().is_none());
}

#[test]
fn runner_take_events_yields_receiver_on_event_hybrid() {
    let (hybrid, tx) = make_hybrid_with_events();
    tx.send(CameraEvent::Disconnected).unwrap();
    let opened = OpenedCamera::from_device(Box::new(hybrid));
    let mut runner = CameraRunner::spawn(opened, RunnerConfig::default()).unwrap();
    let events_rx = runner
        .take_events()
        .expect("hybrid with EventSource should yield events receiver");
    assert!(runner.take_events().is_none(), "second take must be None");
    assert!(
        runner.events().is_none(),
        "events() must be None after take_events()"
    );
    let _ev = events_rx
        .recv_timeout(Duration::from_millis(500))
        .expect("owned events receiver still delivers");
}

// ─────────────────── Overflow policy: E2E through CameraRunner ────────
//
// `make_channel(capacity, policy)` is unit-tested directly in
// `src/runner.rs`, but the full path from `CameraRunner::spawn(opened,
// RunnerConfig { overflow: ..., frames_capacity: ..., .. })` through
// the worker thread to the user-facing `Receiver<Buffer>` was never
// exercised end-to-end. A regression that hard-coded a single policy
// in `spawn`, swapped a pair of `frames_capacity` / `events_capacity`
// arguments, or forgot to wire the relay thread for `DropOldest`
// would silently degrade the policy without any test failing.
//
// These two tests pin the spawn → policy → user channel wiring with
// a sequence-number tag in the first byte of each frame so we can
// distinguish dropped vs. delivered without relying on raw counts.

/// Build a `MockFrameSource` whose Nth pushed frame has its first
/// data byte set to N (truncated to u8). Used to verify ordering
/// invariants without the test depending on exact frame counts.
fn make_sequenced_frame_source(count: u8) -> FrameOnly {
    let mut s = MockFrameSource::new(0);
    for n in 0..count {
        // 4×4 GRAY → 16 bytes; bpp=1 so the first byte is observable.
        let mut buf = mock_frame(4, 4, FrameFormat::GRAY).buffer().to_vec();
        buf[0] = n;
        s.push_frame(Buffer::from_vec(
            nokhwa_core::types::Resolution::new(4, 4),
            buf,
            FrameFormat::GRAY,
        ));
    }
    FrameOnly(s)
}

/// `Overflow::Block` end-to-end: with `frames_capacity = 1` and a
/// finite frame queue, the worker must block on a full channel
/// rather than drop frames. Pushing 4 sequenced frames and draining
/// after the source has stalled (errored on empty queue) must yield
/// exactly those 4 frames in order. A regression that swapped Block
/// for DropOldest / DropNewest would lose frames; a regression that
/// dropped Block to a no-op would fail to produce them at all.
#[test]
fn runner_overflow_block_delivers_every_frame_in_order() {
    let opened = OpenedCamera::from_device(Box::new(make_sequenced_frame_source(4)));
    let cfg = RunnerConfig {
        frames_capacity: 1,
        overflow: Overflow::Block,
        ..RunnerConfig::default()
    };
    let runner = CameraRunner::spawn(opened, cfg).unwrap();
    let rx = runner.frames().expect("stream runner has frames channel");

    let mut received = Vec::new();
    // Pull until the source stalls. Each `recv_timeout` window must
    // be long enough to cover the runner's `poll_interval` (10ms
    // default) plus channel send latency.
    while let Ok(buf) = rx.recv_timeout(Duration::from_millis(500)) {
        received.push(buf.buffer()[0]);
        if received.len() == 4 {
            // Drained the entire pushed sequence; subsequent recv
            // would just time out on the stalled (empty-queue) source.
            break;
        }
    }

    assert_eq!(
        received,
        vec![0, 1, 2, 3],
        "Block must deliver every produced frame in order; got {received:?}"
    );
}

/// `Overflow::DropOldest` end-to-end: the relay thread is wired only
/// when this policy is selected. Pin two invariants — (a) frames
/// arrive in order (DropOldest never reorders), and (b) when the
/// source produces faster than we drain, the *last* frame produced
/// (highest sequence number) must always be observable, because
/// DropOldest evicts the front of the buffer to make room.
///
/// The test uses 16 finite sequenced frames (range fits in u8) and a
/// `frames_capacity = 1` channel. We don't drain during production —
/// we sleep long enough for the runner to walk the entire queue —
/// then drain everything that arrived. The sequence we observe must
/// be strictly monotonic, and 15 (the last frame) must appear at
/// the tail. Doesn't depend on the exact number of survivors, only
/// on the ordering invariant + last-frame survivability.
#[test]
fn runner_overflow_drop_oldest_preserves_order_and_keeps_latest_frame() {
    let opened = OpenedCamera::from_device(Box::new(make_sequenced_frame_source(16)));
    let cfg = RunnerConfig {
        frames_capacity: 1,
        overflow: Overflow::DropOldest,
        ..RunnerConfig::default()
    };
    let runner = CameraRunner::spawn(opened, cfg).unwrap();

    // Let the runner produce ALL 16 frames. With a 10ms poll_interval
    // default + 5ms relay drain tick, 1s is comfortably enough for
    // the source to drain (16 frames × ~10ms each = ~160ms).
    std::thread::sleep(Duration::from_millis(1000));

    let rx = runner.frames().expect("stream runner has frames channel");
    let mut received = Vec::new();
    while let Ok(buf) = rx.recv_timeout(Duration::from_millis(200)) {
        received.push(buf.buffer()[0]);
    }

    assert!(
        !received.is_empty(),
        "DropOldest must surface at least one frame"
    );
    assert!(
        received.windows(2).all(|w| w[0] < w[1]),
        "DropOldest must preserve order; got {received:?}"
    );
    assert_eq!(
        received.last().copied(),
        Some(15),
        "DropOldest must keep the most recent frame at the tail; got {received:?}"
    );
}
