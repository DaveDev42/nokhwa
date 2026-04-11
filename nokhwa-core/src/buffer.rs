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
use crate::types::{FrameFormat, Resolution};
use bytes::Bytes;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Describes the semantics of a capture timestamp.
///
/// Different platforms produce timestamps with different reference clocks.
/// This enum lets callers know what a [`Duration`] actually represents.
#[derive(Clone, Copy, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum TimestampKind {
    /// The timestamp represents when the frame was captured by the sensor.
    Capture,
    /// The timestamp is a presentation timestamp assigned by the media pipeline
    /// (e.g. `CMSampleBuffer` on macOS).
    Presentation,
    /// The timestamp is derived from a monotonic clock
    /// (e.g. `IMFSample` clock on Windows).
    MonotonicClock,
    /// The timestamp is a wall-clock time (e.g. `CLOCK_REALTIME` / `SystemTime`).
    WallClock,
    /// The timestamp source is unknown or not specified by the backend.
    Unknown,
}

/// A buffer returned by a camera to accommodate custom decoding.
/// Contains information of Resolution, the buffer's [`FrameFormat`], and the buffer.
///
/// Note that decoding on the main thread **will** decrease your performance and lead to dropped frames.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Debug, Hash, PartialOrd, PartialEq, Eq)]
pub struct Buffer {
    /// Width and height of the frame.
    pub(crate) resolution: Resolution,
    pub(crate) data: Bytes,
    pub(crate) source_frame_format: FrameFormat,
    pub(crate) capture_timestamp: Option<(Duration, TimestampKind)>,
}

impl Buffer {
    /// Creates a new buffer with a [`&[u8]`].
    #[must_use]
    #[inline]
    pub fn new(res: Resolution, buf: &[u8], source_frame_format: FrameFormat) -> Self {
        Self {
            resolution: res,
            data: Bytes::copy_from_slice(buf),
            source_frame_format,
            capture_timestamp: None,
        }
    }

    /// Creates a new buffer with a [`&[u8]`] and a backend-provided capture timestamp.
    #[must_use]
    #[inline]
    pub fn with_timestamp(
        res: Resolution,
        buf: &[u8],
        source_frame_format: FrameFormat,
        capture_timestamp: Option<(Duration, TimestampKind)>,
    ) -> Self {
        Self {
            resolution: res,
            data: Bytes::copy_from_slice(buf),
            source_frame_format,
            capture_timestamp,
        }
    }

    /// Creates a new buffer taking ownership of a [`Vec<u8>`] without copying.
    #[must_use]
    #[inline]
    pub fn from_vec(res: Resolution, buf: Vec<u8>, source_frame_format: FrameFormat) -> Self {
        Self {
            resolution: res,
            data: Bytes::from(buf),
            source_frame_format,
            capture_timestamp: None,
        }
    }

    /// Creates a new buffer taking ownership of a [`Vec<u8>`] without copying,
    /// with a backend-provided capture timestamp.
    #[must_use]
    #[inline]
    pub fn from_vec_with_timestamp(
        res: Resolution,
        buf: Vec<u8>,
        source_frame_format: FrameFormat,
        capture_timestamp: Option<(Duration, TimestampKind)>,
    ) -> Self {
        Self {
            resolution: res,
            data: Bytes::from(buf),
            source_frame_format,
            capture_timestamp,
        }
    }

    /// Get the backend-provided capture timestamp, if available.
    #[must_use]
    pub fn capture_timestamp(&self) -> Option<Duration> {
        self.capture_timestamp.map(|(ts, _)| ts)
    }

    /// Get the backend-provided capture timestamp and its [`TimestampKind`], if available.
    #[must_use]
    pub fn capture_timestamp_with_kind(&self) -> Option<(Duration, TimestampKind)> {
        self.capture_timestamp
    }

    /// Get the [`Resolution`] of this buffer.
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Get the data of this buffer as a byte slice reference without copying.
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        &self.data
    }

    /// Get an owned version of this buffer.
    #[must_use]
    pub fn buffer_bytes(&self) -> Bytes {
        self.data.clone()
    }

    /// Get the [`FrameFormat`] of this buffer.
    #[must_use]
    pub fn source_frame_format(&self) -> FrameFormat {
        self.source_frame_format
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
