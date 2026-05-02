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

//! Type-safe capture format marker types.
//!
//! Each zero-sized type (ZST) represents a camera pixel format. The [`CaptureFormat`]
//! trait connects these types to the runtime [`FrameFormat`] enum, enabling the compiler
//! to enforce format-specific operations (e.g. preventing `Gray` frames from being
//! converted to RGB).

use crate::types::FrameFormat;

/// Marker trait for camera capture pixel formats.
///
/// Implemented by zero-sized types that represent a specific wire format
/// (e.g. [`Yuyv`], [`Mjpeg`]). The associated [`FRAME_FORMAT`](CaptureFormat::FRAME_FORMAT)
/// constant maps the type to the runtime [`FrameFormat`] enum.
pub trait CaptureFormat: Send + Sync + 'static {
    /// The runtime [`FrameFormat`] this type represents.
    const FRAME_FORMAT: FrameFormat;
}

/// YUYV 4:2:2 packed format.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Yuyv;

impl CaptureFormat for Yuyv {
    const FRAME_FORMAT: FrameFormat = FrameFormat::YUYV;
}

/// NV12 (YUV 4:2:0 bi-planar) format.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Nv12;

impl CaptureFormat for Nv12 {
    const FRAME_FORMAT: FrameFormat = FrameFormat::NV12;
}

/// Motion-JPEG compressed format.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Mjpeg;

impl CaptureFormat for Mjpeg {
    const FRAME_FORMAT: FrameFormat = FrameFormat::MJPEG;
}

/// 8-bit grayscale format.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Gray;

impl CaptureFormat for Gray {
    const FRAME_FORMAT: FrameFormat = FrameFormat::GRAY;
}

/// Raw RGB888 format.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct RawRgb;

impl CaptureFormat for RawRgb {
    const FRAME_FORMAT: FrameFormat = FrameFormat::RAWRGB;
}

/// Raw BGR888 format.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct RawBgr;

impl CaptureFormat for RawBgr {
    const FRAME_FORMAT: FrameFormat = FrameFormat::RAWBGR;
}

#[cfg(test)]
#[path = "format_types_tests.rs"]
mod tests;
