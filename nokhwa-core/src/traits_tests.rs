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

#![cfg(test)]

use crate::buffer::Buffer;
use crate::error::NokhwaError;
use crate::traits::{
    CameraDevice, CameraEvent, EventPoll, EventSource, FrameSource, ShutterCapture,
};
use crate::types::{
    ApiBackend, CameraControl, CameraFormat, CameraInfo, ControlValueSetter, FrameFormat,
    KnownCameraControl, Resolution,
};
use std::borrow::Cow;
use std::time::Duration;

struct Dummy {
    info: CameraInfo,
    open: bool,
}

impl CameraDevice for Dummy {
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
        _value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        Ok(())
    }
}

impl FrameSource for Dummy {
    fn negotiated_format(&self) -> CameraFormat {
        unimplemented!()
    }
    fn set_format(&mut self, _f: CameraFormat) -> Result<(), NokhwaError> {
        Ok(())
    }
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        Ok(vec![])
    }
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        Ok(vec![])
    }
    fn open(&mut self) -> Result<(), NokhwaError> {
        self.open = true;
        Ok(())
    }
    fn is_open(&self) -> bool {
        self.open
    }
    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        unimplemented!()
    }
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        Ok(Cow::Borrowed(&[]))
    }
    fn close(&mut self) -> Result<(), NokhwaError> {
        self.open = false;
        Ok(())
    }
}

impl ShutterCapture for Dummy {
    fn trigger(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }
    fn take_picture(&mut self, _timeout: Duration) -> Result<Buffer, NokhwaError> {
        Err(NokhwaError::TimeoutError(Duration::ZERO))
    }
}

struct DummyEvents;
impl EventPoll for DummyEvents {
    fn try_next(&mut self) -> Option<CameraEvent> {
        None
    }
    fn next_timeout(&mut self, _d: Duration) -> Option<CameraEvent> {
        None
    }
}

impl EventSource for Dummy {
    fn take_events(&mut self) -> Result<Box<dyn EventPoll + Send>, NokhwaError> {
        Ok(Box::new(DummyEvents))
    }
}

fn sample_info() -> CameraInfo {
    use crate::types::CameraIndex;
    CameraInfo::new("dummy", "dummy", "", CameraIndex::Index(0))
}

#[test]
fn dummy_implements_all_capabilities() {
    let mut d = Dummy {
        info: sample_info(),
        open: false,
    };
    let _: &dyn CameraDevice = &d;
    let _: &mut dyn FrameSource = &mut d;
    let _: &mut dyn ShutterCapture = &mut d;
    let _: &mut dyn EventSource = &mut d;
}

#[test]
fn shutter_capture_default_methods() {
    let mut d = Dummy {
        info: sample_info(),
        open: false,
    };
    assert!(d.lock_ui().is_ok());
    assert!(d.unlock_ui().is_ok());
    let r = d.capture(Duration::ZERO);
    assert!(matches!(r, Err(NokhwaError::TimeoutError(_))));
}

struct FormatStub {
    info: CameraInfo,
    fmt: CameraFormat,
}

impl CameraDevice for FormatStub {
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
        _value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        Ok(())
    }
}

impl FrameSource for FormatStub {
    fn negotiated_format(&self) -> CameraFormat {
        self.fmt
    }
    fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
        self.fmt = f;
        Ok(())
    }
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        Ok(vec![])
    }
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        Ok(vec![])
    }
    fn open(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }
    fn is_open(&self) -> bool {
        false
    }
    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        unimplemented!()
    }
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        Ok(Cow::Borrowed(&[]))
    }
    fn close(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }
}

fn stub(format: FrameFormat, w: u32, h: u32) -> FormatStub {
    FormatStub {
        info: sample_info(),
        fmt: CameraFormat::new(Resolution::new(w, h), format, 30),
    }
}

#[test]
fn decoded_buffer_size_three_byte_formats_alpha_off() {
    for f in [
        FrameFormat::MJPEG,
        FrameFormat::YUYV,
        FrameFormat::RAWRGB,
        FrameFormat::RAWBGR,
        FrameFormat::NV12,
    ] {
        let s = stub(f, 640, 480);
        assert_eq!(
            s.decoded_buffer_size(false),
            640 * 480 * 3,
            "format {f:?} alpha=false"
        );
    }
}

#[test]
fn decoded_buffer_size_three_byte_formats_alpha_on() {
    for f in [
        FrameFormat::MJPEG,
        FrameFormat::YUYV,
        FrameFormat::RAWRGB,
        FrameFormat::RAWBGR,
        FrameFormat::NV12,
    ] {
        let s = stub(f, 320, 240);
        assert_eq!(
            s.decoded_buffer_size(true),
            320 * 240 * 4,
            "format {f:?} alpha=true"
        );
    }
}

#[test]
fn decoded_buffer_size_gray_alpha_off_is_one_byte_per_pixel() {
    let s = stub(FrameFormat::GRAY, 1920, 1080);
    assert_eq!(s.decoded_buffer_size(false), 1920 * 1080);
}

#[test]
fn decoded_buffer_size_gray_alpha_on_is_two_bytes_per_pixel() {
    let s = stub(FrameFormat::GRAY, 1920, 1080);
    assert_eq!(s.decoded_buffer_size(true), 1920 * 1080 * 2);
}

struct FrameCallCounter {
    info: CameraInfo,
    fmt: CameraFormat,
    frame_calls: u32,
}

impl CameraDevice for FrameCallCounter {
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
        _value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        Ok(())
    }
}

impl FrameSource for FrameCallCounter {
    fn negotiated_format(&self) -> CameraFormat {
        self.fmt
    }
    fn set_format(&mut self, _f: CameraFormat) -> Result<(), NokhwaError> {
        Ok(())
    }
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        Ok(vec![])
    }
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        Ok(vec![])
    }
    fn open(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }
    fn is_open(&self) -> bool {
        false
    }
    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.frame_calls += 1;
        Ok(Buffer::new(self.fmt.resolution(), &[], self.fmt.format()))
    }
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        Ok(Cow::Borrowed(&[]))
    }
    fn close(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }
}

#[test]
fn frame_timeout_default_forwards_to_frame() {
    let mut c = FrameCallCounter {
        info: sample_info(),
        fmt: CameraFormat::new(Resolution::new(640, 480), FrameFormat::YUYV, 30),
        frame_calls: 0,
    };
    let _ = c.frame_timeout(Duration::from_millis(50));
    assert_eq!(c.frame_calls, 1);
    let _ = c.frame_timeout(Duration::from_millis(50));
    assert_eq!(c.frame_calls, 2);
}
