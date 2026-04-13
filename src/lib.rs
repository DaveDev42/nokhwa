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
//! A cross-platform Rust library for camera capture. Supports webcam streaming
//! (V4L2, `AVFoundation`, Media Foundation) and plug-in backends for cameras
//! with distinct capture models (DSLR/industrial via external crates).
//!
//! ## Quick start
//!
//! ```no_run
//! use nokhwa::{CameraSession, OpenRequest, OpenedCamera};
//! use nokhwa_core::types::CameraIndex;
//!
//! # fn main() -> Result<(), nokhwa_core::error::NokhwaError> {
//! let req = OpenRequest::any();
//! match CameraSession::open(CameraIndex::Index(0), req)? {
//!     OpenedCamera::Stream(mut cam) => {
//!         cam.open()?;
//!         let frame = cam.frame()?;
//!         println!(
//!             "captured {}x{}",
//!             frame.resolution().width(),
//!             frame.resolution().height()
//!         );
//!         cam.close()?;
//!     }
//!     OpenedCamera::Shutter(mut cam) => {
//!         let photo = cam.capture(std::time::Duration::from_secs(5))?;
//!         println!("photo: {} bytes", photo.buffer().len());
//!     }
//!     OpenedCamera::Hybrid(mut cam) => {
//!         cam.open()?;
//!         let _preview = cam.frame()?;
//!         let _photo = cam.capture(std::time::Duration::from_secs(5))?;
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! For apps that need live-view + pictures + events concurrently, see
//! [`CameraRunner`] (feature = `runner`).
//!
//! ## Feature flags
//!
//! Enable at least one input backend: `input-native` (auto-selects), or
//! `input-v4l` / `input-avfoundation` / `input-msmf` for a specific platform.
//!
//! - `mjpeg` (default): MJPEG decoding via `mozjpeg`.
//! - `runner`: threaded helper [`CameraRunner`].
//! - `output-wgpu`: direct frame-to-wgpu texture copy.
//! - `serialize`: serde on core types.
//! - `logging`: route internal diagnostics through the `log` crate.
//!
//! ## Traits
//!
//! Backends implement [`CameraDevice`](nokhwa_core::traits::CameraDevice) plus
//! any of [`FrameSource`](nokhwa_core::traits::FrameSource),
//! [`ShutterCapture`](nokhwa_core::traits::ShutterCapture),
//! [`EventSource`](nokhwa_core::traits::EventSource).

// input-opencv backend is pending migration to the 0.13.0 trait split.
#[cfg(feature = "input-opencv")]
compile_error!(
    "input-opencv backend is pending migration to the 0.13.0 trait split. \
     Track progress in TODO.md."
);

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
mod init;
mod query;

// Layer 2: CameraSession / OpenedCamera and the per-capability wrappers.
// Layer 3 (CameraRunner) lives in the `runner` module below, gated on the
// `runner` feature.
pub mod session;
pub use session::{
    CameraSession, HybridCamera, OpenRequest, OpenedCamera, ShutterCamera, StreamCamera,
};

#[cfg(feature = "runner")]
pub mod runner;
#[cfg(feature = "runner")]
pub use runner::{CameraRunner, RunnerConfig};

pub use init::*;
pub use nokhwa_core::buffer::{Buffer, TimestampKind};
pub use nokhwa_core::error::NokhwaError;
pub use nokhwa_core::format_types;
pub use nokhwa_core::frame;
#[cfg(feature = "output-wgpu")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-wgpu")))]
pub use nokhwa_core::wgpu::{raw_texture_layout, RawTextureData};
pub use query::*;

pub mod utils {
    pub use nokhwa_core::types::*;
}

pub mod error {
    pub use nokhwa_core::error::NokhwaError;
}

pub mod buffer {
    pub use nokhwa_core::buffer::*;
}
