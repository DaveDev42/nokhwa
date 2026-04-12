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

//! Mock backend implementations of the capability traits for use in tests.
//!
//! Enable with the `testing` feature. These types are intentionally minimal
//! and deterministic; they perform no I/O.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::buffer::Buffer;
use crate::error::NokhwaError;
use crate::traits::{
    CameraDevice, CameraEvent, EventPoll, EventSource, FrameSource, ShutterCapture,
};
use crate::types::{
    ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
    FrameFormat, KnownCameraControl, Resolution,
};

/// Build a deterministic [`CameraInfo`] for tests.
#[must_use]
pub fn mock_info(index: u32) -> CameraInfo {
    CameraInfo::new(
        "Mock Camera",
        "mock camera for tests",
        "mock",
        CameraIndex::Index(index),
    )
}

/// Build a deterministic [`Buffer`] of the given shape. The payload is a
/// zero-filled vector sized to `w * h * bpp`, where `bpp` is a plausible
/// bytes-per-pixel for the chosen format.
///
/// The per-format `bpp` values here are intentionally coarse and meant only
/// for test fixtures — the real encoded byte counts (especially for MJPEG
/// and sub-sampled YUV formats) differ and should not be inferred from this
/// helper.
#[must_use]
pub fn mock_frame(width: u32, height: u32, format: FrameFormat) -> Buffer {
    let bpp: usize = match format {
        FrameFormat::MJPEG
        | FrameFormat::YUYV
        | FrameFormat::RAWRGB
        | FrameFormat::RAWBGR
        | FrameFormat::NV12 => 3,
        FrameFormat::GRAY => 1,
    };
    let len = (width as usize) * (height as usize) * bpp;
    let data = vec![0u8; len];
    Buffer::from_vec(Resolution::new(width, height), data, format)
}

fn default_format() -> CameraFormat {
    CameraFormat::new(Resolution::new(640, 480), FrameFormat::YUYV, 30)
}

/// A simple continuous-frame mock backend.
pub struct MockFrameSource {
    info: CameraInfo,
    format: CameraFormat,
    is_open: bool,
    queue: VecDeque<Buffer>,
}

impl MockFrameSource {
    /// Create a new mock with an empty frame queue.
    #[must_use]
    pub fn new(index: u32) -> Self {
        Self {
            info: mock_info(index),
            format: default_format(),
            is_open: false,
            queue: VecDeque::new(),
        }
    }

    /// Push a frame onto the queue. `frame()` returns frames in FIFO order.
    pub fn push_frame(&mut self, frame: Buffer) {
        self.queue.push_back(frame);
    }
}

impl Default for MockFrameSource {
    fn default() -> Self {
        Self::new(0)
    }
}

impl CameraDevice for MockFrameSource {
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

impl FrameSource for MockFrameSource {
    fn negotiated_format(&self) -> CameraFormat {
        self.format
    }
    fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
        self.format = f;
        Ok(())
    }
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        Ok(vec![self.format])
    }
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        Ok(vec![self.format.format()])
    }

    fn open(&mut self) -> Result<(), NokhwaError> {
        self.is_open = true;
        Ok(())
    }
    fn is_open(&self) -> bool {
        self.is_open
    }
    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        if let Some(buf) = self.queue.pop_front() {
            Ok(buf)
        } else {
            Err(NokhwaError::TimeoutError(Duration::ZERO))
        }
    }
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        match self.queue.pop_front() {
            Some(buf) => Ok(Cow::Owned(buf.buffer().to_vec())),
            None => Err(NokhwaError::TimeoutError(Duration::ZERO)),
        }
    }
    fn close(&mut self) -> Result<(), NokhwaError> {
        self.is_open = false;
        Ok(())
    }
}

/// A simple shutter-capture mock backend.
pub struct MockShutter {
    info: CameraInfo,
    triggered: VecDeque<Buffer>,
    pending: VecDeque<Buffer>,
}

impl MockShutter {
    /// Create a new mock. `pictures` is the FIFO pool that each `trigger()`
    /// pulls from, enqueuing a picture for the next `take_picture`.
    #[must_use]
    pub fn new(pictures: Vec<Buffer>) -> Self {
        Self {
            info: mock_info(0),
            triggered: pictures.into(),
            pending: VecDeque::new(),
        }
    }
}

impl CameraDevice for MockShutter {
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

impl ShutterCapture for MockShutter {
    fn trigger(&mut self) -> Result<(), NokhwaError> {
        if let Some(pic) = self.triggered.pop_front() {
            self.pending.push_back(pic);
        }
        Ok(())
    }
    fn take_picture(&mut self, timeout: Duration) -> Result<Buffer, NokhwaError> {
        if let Some(pic) = self.pending.pop_front() {
            Ok(pic)
        } else {
            Err(NokhwaError::TimeoutError(timeout))
        }
    }
}

/// Hybrid mock that combines [`FrameSource`] and [`ShutterCapture`].
pub struct MockHybrid {
    frames: MockFrameSource,
    shutter: MockShutter,
}

impl MockHybrid {
    #[must_use]
    pub fn new(index: u32, pictures: Vec<Buffer>) -> Self {
        Self {
            frames: MockFrameSource::new(index),
            shutter: MockShutter::new(pictures),
        }
    }

    pub fn push_frame(&mut self, frame: Buffer) {
        self.frames.push_frame(frame);
    }
}

impl CameraDevice for MockHybrid {
    fn backend(&self) -> ApiBackend {
        self.frames.backend()
    }
    fn info(&self) -> &CameraInfo {
        self.frames.info()
    }
    fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        self.frames.controls()
    }
    fn set_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.frames.set_control(id, value)
    }
}

impl FrameSource for MockHybrid {
    fn negotiated_format(&self) -> CameraFormat {
        self.frames.negotiated_format()
    }
    fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
        self.frames.set_format(f)
    }
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        self.frames.compatible_formats()
    }
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        self.frames.compatible_fourcc()
    }
    fn open(&mut self) -> Result<(), NokhwaError> {
        self.frames.open()
    }
    fn is_open(&self) -> bool {
        self.frames.is_open()
    }
    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.frames.frame()
    }
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        self.frames.frame_raw()
    }
    fn close(&mut self) -> Result<(), NokhwaError> {
        self.frames.close()
    }
}

impl ShutterCapture for MockHybrid {
    fn trigger(&mut self) -> Result<(), NokhwaError> {
        self.shutter.trigger()
    }
    fn take_picture(&mut self, timeout: Duration) -> Result<Buffer, NokhwaError> {
        self.shutter.take_picture(timeout)
    }
}

/// An [`EventPoll`] implementation backed by an [`std::sync::mpsc`] receiver.
pub struct MpscEventPoll {
    rx: Receiver<CameraEvent>,
}

impl MpscEventPoll {
    #[must_use]
    pub fn new(rx: Receiver<CameraEvent>) -> Self {
        Self { rx }
    }
}

impl EventPoll for MpscEventPoll {
    fn try_next(&mut self) -> Option<CameraEvent> {
        self.rx.try_recv().ok()
    }
    fn next_timeout(&mut self, d: Duration) -> Option<CameraEvent> {
        self.rx.recv_timeout(d).ok()
    }
}

/// A [`FrameSource`] + [`EventSource`] mock, useful for testing event delivery.
pub struct MockEventfulFrameSource {
    inner: MockFrameSource,
    poll: Option<Box<dyn EventPoll + Send>>,
}

impl MockEventfulFrameSource {
    /// Create a new mock that will hand out `poll` on the first (and only)
    /// call to [`EventSource::take_events`].
    #[must_use]
    pub fn new(index: u32, poll: Box<dyn EventPoll + Send>) -> Self {
        Self {
            inner: MockFrameSource::new(index),
            poll: Some(poll),
        }
    }

    pub fn push_frame(&mut self, frame: Buffer) {
        self.inner.push_frame(frame);
    }
}

impl CameraDevice for MockEventfulFrameSource {
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

impl FrameSource for MockEventfulFrameSource {
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

impl EventSource for MockEventfulFrameSource {
    fn take_events(&mut self) -> Result<Box<dyn EventPoll + Send>, NokhwaError> {
        self.poll
            .take()
            .ok_or(NokhwaError::UnsupportedOperationError(ApiBackend::Browser))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn frame_source_returns_pushed_frames_in_order() {
        let mut src = MockFrameSource::new(0);
        assert!(!src.is_open());
        src.open().unwrap();
        assert!(src.is_open());
        src.push_frame(mock_frame(4, 4, FrameFormat::YUYV));
        src.push_frame(mock_frame(8, 8, FrameFormat::YUYV));
        let a = src.frame().unwrap();
        let b = src.frame().unwrap();
        assert_eq!(a.resolution(), Resolution::new(4, 4));
        assert_eq!(b.resolution(), Resolution::new(8, 8));
        assert!(matches!(
            src.frame(),
            Err(NokhwaError::TimeoutError(d)) if d == Duration::ZERO
        ));
        src.close().unwrap();
        assert!(!src.is_open());
    }

    #[test]
    fn shutter_triggers_and_takes_pictures() {
        let pics = vec![
            mock_frame(2, 2, FrameFormat::MJPEG),
            mock_frame(3, 3, FrameFormat::MJPEG),
        ];
        let mut sh = MockShutter::new(pics);
        assert!(matches!(
            sh.take_picture(Duration::ZERO),
            Err(NokhwaError::TimeoutError(_))
        ));
        sh.trigger().unwrap();
        let p = sh.take_picture(Duration::from_millis(10)).unwrap();
        assert_eq!(p.resolution(), Resolution::new(2, 2));
    }

    #[test]
    fn mpsc_poll_delivers_events() {
        let (tx, rx) = channel();
        let mut poll = MpscEventPoll::new(rx);
        assert!(poll.try_next().is_none());
        tx.send(CameraEvent::WillShutDown).unwrap();
        assert!(matches!(poll.try_next(), Some(CameraEvent::WillShutDown)));
        tx.send(CameraEvent::Disconnected).unwrap();
        assert!(matches!(
            poll.next_timeout(Duration::from_millis(50)),
            Some(CameraEvent::Disconnected)
        ));
        drop(tx);
        assert!(poll.next_timeout(Duration::from_millis(5)).is_none());
    }

    #[test]
    fn eventful_source_hands_out_poll_once() {
        let (_tx, rx) = channel();
        let poll: Box<dyn EventPoll + Send> = Box::new(MpscEventPoll::new(rx));
        let mut src = MockEventfulFrameSource::new(0, poll);
        assert!(src.take_events().is_ok());
        assert!(matches!(
            src.take_events(),
            Err(NokhwaError::UnsupportedOperationError(_))
        ));
    }
}
