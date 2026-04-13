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
use nokhwa_core::testing::{mock_frame, MockFrameSource, MockHybrid, MockShutter};
use nokhwa_core::traits::{CameraDevice, FrameSource, ShutterCapture};
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
fn shutter_camera_capture_wraps_lock_trigger_take_unlock() {
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

/// M12: ensures the V4L branch in [`CameraSession::open`] stays stubbed
/// until it is intentionally re-enabled in 0.13.1. When the dispatch path
/// is rewired, this test will start failing and the assertion should be
/// deleted along with the stub.
#[cfg(all(target_os = "linux", feature = "input-v4l"))]
#[test]
fn camera_session_open_v4l_is_stubbed_in_0_13_0() {
    use nokhwa::{CameraSession, OpenRequest};
    use nokhwa_core::types::CameraIndex;

    let err = CameraSession::open(CameraIndex::Index(0), OpenRequest::any())
        .expect_err("V4L path must be stubbed in 0.13.0");
    let msg = format!("{err}");
    assert!(
        msg.contains("0.13.1"),
        "stub error should reference 0.13.1 deferral, got: {msg}"
    );
}
