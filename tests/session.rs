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

//! Integration tests for [`nokhwa::session`].
//!
//! Each mock from `nokhwa_core::testing` is wrapped in a local newtype so
//! the orphan rule lets us invoke [`nokhwa_backend!`] on it from this
//! integration-test crate.

use std::borrow::Cow;
use std::time::Duration;

use nokhwa::nokhwa_backend;
use nokhwa::{HybridCamera, OpenedCamera, ShutterCamera, StreamCamera};
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

struct Hybrid(MockHybrid);

impl CameraDevice for Hybrid {
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
impl FrameSource for Hybrid {
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
impl ShutterCapture for Hybrid {
    fn trigger(&mut self) -> Result<(), NokhwaError> {
        self.0.trigger()
    }
    fn take_picture(&mut self, t: Duration) -> Result<Buffer, NokhwaError> {
        self.0.take_picture(t)
    }
}

nokhwa_backend!(FrameOnly: FrameSource);
nokhwa_backend!(ShutterOnly: ShutterCapture);
nokhwa_backend!(Hybrid: FrameSource, ShutterCapture);

fn make_shutter() -> ShutterOnly {
    ShutterOnly(MockShutter::new(vec![
        mock_frame(4, 4, FrameFormat::MJPEG),
        mock_frame(4, 4, FrameFormat::MJPEG),
    ]))
}

fn make_hybrid() -> Hybrid {
    let mut h = MockHybrid::new(0, vec![mock_frame(4, 4, FrameFormat::MJPEG)]);
    h.push_frame(mock_frame(8, 8, FrameFormat::YUYV));
    Hybrid(h)
}

#[test]
fn mock_frame_source_wraps_as_stream_variant() {
    let opened = OpenedCamera::from_device(Box::new(FrameOnly(MockFrameSource::new(0))));
    assert!(matches!(opened, OpenedCamera::Stream(_)));
}

#[test]
fn mock_shutter_wraps_as_shutter_variant() {
    let opened = OpenedCamera::from_device(Box::new(make_shutter()));
    assert!(matches!(opened, OpenedCamera::Shutter(_)));
}

#[test]
fn mock_hybrid_wraps_as_hybrid_variant() {
    let opened = OpenedCamera::from_device(Box::new(make_hybrid()));
    assert!(matches!(opened, OpenedCamera::Hybrid(_)));
}

#[test]
fn stream_camera_open_frame_close_cycle() {
    let mut src = MockFrameSource::new(0);
    src.push_frame(mock_frame(4, 4, FrameFormat::YUYV));
    let mut cam = StreamCamera::from_device(Box::new(FrameOnly(src)));
    assert!(cam.open().is_ok());
    assert!(cam.is_open());
    assert!(cam.frame().is_ok());
    assert!(cam.close().is_ok());
    assert!(!cam.is_open());
}

#[test]
fn shutter_camera_capture_wraps_lock_ui_trigger_take_unlock_ui() {
    let mut cam = ShutterCamera::from_device(Box::new(make_shutter()));
    let photo = cam.capture(Duration::from_millis(100));
    assert!(photo.is_ok());
}

#[test]
fn hybrid_camera_exposes_both_surfaces() {
    let mut cam = HybridCamera::from_device(Box::new(make_hybrid()));
    assert!(cam.open().is_ok());
    let _ = cam.frame().unwrap();
    let _ = cam.capture(Duration::from_millis(100)).unwrap();
}

#[test]
fn hybrid_camera_without_events_returns_none() {
    let mut cam = HybridCamera::from_device(Box::new(make_hybrid()));
    assert!(cam.take_events().is_none());
}

// ─────────────────── External FrameSource+ShutterCapture+EventSource ──────
//
// Proves that a downstream crate can declare all three capabilities on the
// same type via `nokhwa_backend!` and have `HybridCamera::from_device` wire
// the event poller through. Exercises the `EventSource` arm of the macro,
// which the other newtype tests do not.

struct EventfulHybrid {
    inner: MockHybrid,
    poll: Option<Box<dyn EventPoll + Send>>,
}

impl CameraDevice for EventfulHybrid {
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
impl FrameSource for EventfulHybrid {
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
impl ShutterCapture for EventfulHybrid {
    fn trigger(&mut self) -> Result<(), NokhwaError> {
        self.inner.trigger()
    }
    fn take_picture(&mut self, t: Duration) -> Result<Buffer, NokhwaError> {
        self.inner.take_picture(t)
    }
}
impl EventSource for EventfulHybrid {
    fn take_events(&mut self) -> Result<Box<dyn EventPoll + Send>, NokhwaError> {
        // `OpenedCamera::from_device` calls this exactly once while building
        // the `HybridCamera`, which then memoises the poller — so the test
        // never re-enters this method.
        Ok(self
            .poll
            .take()
            .expect("take_events called twice; HybridCamera should cache"))
    }
}

nokhwa_backend!(EventfulHybrid: FrameSource, ShutterCapture, EventSource);

#[test]
fn hybrid_camera_with_events_delivers_poller() {
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(CameraEvent::WillShutDown).unwrap();

    let hybrid = MockHybrid::new(0, vec![mock_frame(4, 4, FrameFormat::MJPEG)]);
    let dev = EventfulHybrid {
        inner: hybrid,
        poll: Some(Box::new(MpscEventPoll::new(rx))),
    };

    let opened = OpenedCamera::from_device(Box::new(dev));
    let OpenedCamera::Hybrid(mut cam) = opened else {
        panic!("expected hybrid variant");
    };

    let mut poll = cam
        .take_events()
        .expect("events present")
        .expect("poll constructed");
    assert!(matches!(
        poll.next_timeout(Duration::from_millis(50)),
        Some(CameraEvent::WillShutDown)
    ));
    // Subsequent take_events call returns None (poller already taken).
    assert!(cam.take_events().is_none());
}

// ─────────────────── from_device capability-assertion panics ──────────────
//
// Pin the documented `# Panics` contracts on `OpenedCamera::from_device`,
// `StreamCamera::from_device`, `ShutterCamera::from_device`, and
// `HybridCamera::from_device`. Each wrapper asserts that the `AnyDevice`
// advertises the right `CAP_*` bits before downcasting; passing a mismatched
// device must panic with the documented message rather than reaching the
// `unreachable!()` placeholder inside `nokhwa_backend!`.

/// `AnyDevice` that advertises zero capabilities. Used only to drive the
/// `(false, false)` panic arm of `OpenedCamera::from_device`.
struct NoCaps;

impl nokhwa::session::AnyDevice for NoCaps {
    fn capabilities(&self) -> u32 {
        0
    }
    fn into_frame_source(self: Box<Self>) -> Box<dyn FrameSource + Send> {
        unreachable!("NoCaps has no FrameSource")
    }
    fn into_shutter(self: Box<Self>) -> Box<dyn ShutterCapture + Send> {
        unreachable!("NoCaps has no ShutterCapture")
    }
    fn into_hybrid(self: Box<Self>) -> Box<dyn nokhwa::session::HybridBackend + Send> {
        unreachable!("NoCaps is not hybrid")
    }
    fn take_events(&mut self) -> Option<Result<Box<dyn EventPoll + Send>, NokhwaError>> {
        None
    }
}

#[test]
#[should_panic(expected = "advertises no capabilities")]
fn opened_camera_from_device_panics_on_zero_caps() {
    let _ = OpenedCamera::from_device(Box::new(NoCaps));
}

#[test]
#[should_panic(expected = "StreamCamera requires a FrameSource-capable backend")]
fn stream_camera_from_device_panics_without_cap_frame() {
    // `ShutterOnly` advertises CAP_SHUTTER but not CAP_FRAME.
    let _ = StreamCamera::from_device(Box::new(make_shutter()));
}

#[test]
#[should_panic(expected = "ShutterCamera requires a ShutterCapture-capable backend")]
fn shutter_camera_from_device_panics_without_cap_shutter() {
    // `FrameOnly` advertises CAP_FRAME but not CAP_SHUTTER.
    let _ = ShutterCamera::from_device(Box::new(FrameOnly(MockFrameSource::new(0))));
}

#[test]
#[should_panic(expected = "HybridCamera requires both FrameSource and ShutterCapture")]
fn hybrid_camera_from_device_panics_without_both_caps() {
    // `FrameOnly` only has CAP_FRAME, missing CAP_SHUTTER.
    let _ = HybridCamera::from_device(Box::new(FrameOnly(MockFrameSource::new(0))));
}

#[test]
#[should_panic(expected = "HybridCamera requires both FrameSource and ShutterCapture")]
fn hybrid_camera_from_device_panics_with_shutter_only() {
    // `ShutterOnly` only has CAP_SHUTTER, missing CAP_FRAME.
    let _ = HybridCamera::from_device(Box::new(make_shutter()));
}
