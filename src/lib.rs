#![deny(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]
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
#![cfg_attr(feature = "docs-features", feature(doc_cfg))]
//! # nokhwa (녹화)
//!
//! A cross-platform Rust library for webcam capture.
//!
//! Nokhwa provides a unified [`Camera`] API that abstracts over platform-specific
//! backends so you can write camera code once and run it on Linux, macOS, and Windows.
//!
//! The recommended default feature to enable is `input-native` (also available as `input-auto`).
//! The library will not work without at least one `input-*` feature enabled.
//!
//! ## Quick start
//!
//! `Camera<F>` is parameterized by a [`CaptureFormat`](nokhwa_core::format_types::CaptureFormat)
//! marker type. The format `F` is checked against the hardware at open time,
//! and every [`Frame<F>`](nokhwa_core::frame::Frame) produced by the camera
//! carries that tag at compile time.
//!
//! ```no_run
//! use nokhwa::Camera;
//! use nokhwa::utils::{CameraIndex, RequestedFormatType};
//! use nokhwa_core::format_types::Yuyv;
//! use nokhwa_core::frame::IntoRgb;
//!
//! // Open the first camera capturing YUYV at the highest available resolution.
//! let mut camera = Camera::open::<Yuyv>(
//!     CameraIndex::Index(0),
//!     RequestedFormatType::AbsoluteHighestResolution,
//! )?;
//!
//! // Start the stream and grab a typed frame.
//! camera.open_stream()?;
//! let frame = camera.frame_typed()?;                 // Frame<Yuyv>
//! println!("captured {}x{}", frame.resolution().width(), frame.resolution().height());
//!
//! // Lazy conversion: `into_rgb()` is infallible and cheap; `materialize()`
//! // runs the actual pixel conversion.
//! let image = frame.into_rgb().materialize()?;       // ImageBuffer<Rgb<u8>>
//! # Ok::<(), nokhwa::NokhwaError>(())
//! ```
//!
//! Invalid conversions are rejected by the compiler. `Frame<Gray>` does not
//! implement [`IntoRgb`](nokhwa_core::frame::IntoRgb), so this will not
//! compile:
//!
//! ```compile_fail
//! use nokhwa_core::format_types::Gray;
//! use nokhwa_core::frame::{Frame, IntoRgb};
//!
//! fn demo(frame: Frame<Gray>) {
//!     let _ = frame.into_rgb();   // error[E0277]: not satisfied
//! }
//! ```
//!
//! ## Feature flags
//!
//! You **must** enable at least one `input-*` feature for the library to be functional.
//!
//! ### Backend selection
//!
//! | Feature              | Description                                          |
//! |----------------------|------------------------------------------------------|
//! | `input-native`       | Meta-feature: selects the right backend per OS       |
//! | `input-v4l`          | `Video4Linux` backend (Linux)                        |
//! | `input-avfoundation` | `AVFoundation` backend (macOS / iOS)                 |
//! | `input-msmf`         | Media Foundation backend (Windows)                   |
//! | `input-opencv`       | `OpenCV` backend (cross-platform)                    |
//!
//! ### Output / extras
//!
//! | Feature            | Description                                            |
//! |--------------------|--------------------------------------------------------|
//! | `mjpeg`            | MJPEG decoding via `mozjpeg` (enabled by default)      |
//! | `output-threaded`  | [`CallbackCamera`] — background capture with callbacks |
//! | `output-wgpu`      | Direct frame-to-wgpu texture copy                      |
//!
//! ## Key types
//!
//! - [`Camera`] — main capture struct, generic over `F: CaptureFormat`. Start here.
//! - [`CallbackCamera`] — background-thread capture with user callbacks
//!   (`output-threaded` feature); also generic over `F`.
//! - [`Frame`](nokhwa_core::frame::Frame) — type-safe frame handle. Tagged with a
//!   [`CaptureFormat`](nokhwa_core::format_types::CaptureFormat) so invalid
//!   conversions are compile errors.
//! - [`Buffer`] — raw frame payload plus resolution, source
//!   [`FrameFormat`](nokhwa_core::types::FrameFormat), and capture timestamp.
//! - [`CaptureBackendTrait`](crate::camera_traits::CaptureBackendTrait) — the
//!   trait every platform backend implements.
//! - [`CaptureFormat`](nokhwa_core::format_types::CaptureFormat) marker types:
//!   [`Yuyv`](nokhwa_core::format_types::Yuyv),
//!   [`Nv12`](nokhwa_core::format_types::Nv12),
//!   [`Mjpeg`](nokhwa_core::format_types::Mjpeg),
//!   [`Gray`](nokhwa_core::format_types::Gray),
//!   [`RawRgb`](nokhwa_core::format_types::RawRgb),
//!   [`RawBgr`](nokhwa_core::format_types::RawBgr).
//! - Conversion traits: [`IntoRgb`](nokhwa_core::frame::IntoRgb),
//!   [`IntoRgba`](nokhwa_core::frame::IntoRgba),
//!   [`IntoLuma`](nokhwa_core::frame::IntoLuma) — call these on a `Frame<F>`,
//!   then `materialize()` to produce an [`image::ImageBuffer`].
//!
//! ## Backend access
//!
//! The raw backend structs are available in [`backends`] if you need
//! platform-specific functionality beyond what [`Camera`] exposes.

// Ensure at least one input backend is enabled (skip during docs-only builds).
#[cfg(not(feature = "docs-only"))]
#[cfg(not(any(
    feature = "input-avfoundation",
    feature = "input-v4l",
    feature = "input-msmf",
    feature = "input-opencv"
)))]
compile_error!(
    "nokhwa requires at least one input-* feature to be enabled \
     (e.g. input-native / input-auto, input-avfoundation, input-v4l, input-msmf, input-opencv)"
);

/// Raw access to each of Nokhwa's backends.
pub mod backends;
mod camera;
mod init;
mod query;
/// A camera that runs in a different thread and can call your code based on callbacks.
#[cfg(feature = "output-threaded")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-threaded")))]
pub mod threaded;

pub use camera::Camera;
pub use init::*;
pub use nokhwa_core::buffer::{Buffer, TimestampKind};
pub use nokhwa_core::error::NokhwaError;
pub use nokhwa_core::format_types;
pub use nokhwa_core::frame;
#[cfg(feature = "output-wgpu")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-wgpu")))]
pub use nokhwa_core::traits::{raw_texture_layout, RawTextureData};
pub use query::*;
#[cfg(feature = "output-threaded")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-threaded")))]
pub use threaded::CallbackCamera;

pub mod utils {
    pub use nokhwa_core::types::*;
}

pub mod error {
    pub use nokhwa_core::error::NokhwaError;
}

pub mod camera_traits {
    pub use nokhwa_core::traits::*;
}

pub mod buffer {
    pub use nokhwa_core::buffer::*;
}
