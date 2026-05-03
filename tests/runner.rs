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
