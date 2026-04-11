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
//! ## Quick start
//!
//! ```no_run
//! use nokhwa::Camera;
//! use nokhwa::pixel_format::RgbFormat;
//! use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
//!
//! // Open the first camera at its highest resolution.
//! let mut camera = Camera::new(
//!     CameraIndex::Index(0),
//!     RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestResolution),
//! )?;
//!
//! // Start the stream and grab a frame.
//! camera.open_stream()?;
//! let frame = camera.frame()?;
//! println!("captured {}x{}", frame.resolution().width(), frame.resolution().height());
//!
//! // Decode to an `image` RgbImage.
//! let image = frame.decode_image::<RgbFormat>()?;
//! # Ok::<(), nokhwa::NokhwaError>(())
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
//! | `decoding`         | MJPEG decoding via `mozjpeg` (enabled by default)      |
//! | `output-threaded`  | [`CallbackCamera`] — background capture with callbacks |
//! | `output-wgpu`      | Direct frame-to-wgpu texture copy                      |
//!
//! ## Key types
//!
//! - [`Camera`] — main capture struct (start here)
//! - [`CallbackCamera`](crate::threaded::CallbackCamera) — callback-based background capture (`output-threaded`)
//! - [`Buffer`] — raw frame data with metadata; decode via [`Buffer::decode_image`]
//! - [`CaptureBackendTrait`](crate::camera_traits::CaptureBackendTrait) — trait implemented by every backend
//! - [`RequestedFormat`](crate::utils::RequestedFormat) — describes desired camera format
//! - [`CameraFormat`](crate::utils::CameraFormat) — concrete resolution + frame rate + pixel format
//!
//! ## Backend access
//!
//! The raw backend structs are available in [`backends`] if you need
//! platform-specific functionality beyond what [`Camera`] exposes.

/// Raw access to each of Nokhwa's backends.
pub mod backends;
mod camera;
mod init;

pub use nokhwa_core::pixel_format::FormatDecoder;
mod query;
/// A camera that runs in a different thread and can call your code based on callbacks.
#[cfg(feature = "output-threaded")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-threaded")))]
pub mod threaded;

pub use camera::Camera;
pub use init::*;
pub use nokhwa_core::buffer::{Buffer, TimestampKind};
pub use nokhwa_core::error::NokhwaError;
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

pub mod pixel_format {
    pub use nokhwa_core::pixel_format::*;
}

pub mod buffer {
    pub use nokhwa_core::buffer::*;
}
