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
//! use nokhwa::{open, OpenRequest, OpenedCamera};
//! use nokhwa_core::types::CameraIndex;
//!
//! # fn main() -> Result<(), nokhwa_core::error::NokhwaError> {
//! let req = OpenRequest::any();
//! match open(CameraIndex::Index(0), req)? {
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
//! ## Using nokhwa from async runtimes (tokio, async-std, …)
//!
//! [`CameraRunner`] is a **sync** helper: its accessors return
//! [`std::sync::mpsc::Receiver`]s whose `recv()` blocks. That's fine from a
//! non-async program, and it works from any async runtime if you wrap the
//! blocking calls in the runtime's blocking-task helper (e.g.
//! [`tokio::task::spawn_blocking`] or `async_std::task::spawn_blocking`).
//!
//! For tokio users, the companion workspace crate `nokhwa-tokio` ships
//! `TokioCameraRunner`, an async wrapper whose receivers are
//! `tokio::sync::mpsc::Receiver`s (use `.recv().await`). It also handles
//! async-safe `Drop`: dropping the wrapper inside a tokio runtime does not
//! block the caller — the underlying worker thread is joined on a
//! `spawn_blocking` task. Add it as a path or git dependency alongside
//! `nokhwa` (this fork is not published to crates.io).
//!
//! Bounded channels (with `Overflow::DropNewest` or `DropOldest`) are the
//! default for new `CameraRunner`s in 0.14+. A slow async consumer no
//! longer grows memory without bound; the oldest-or-newest frame is
//! dropped according to the configured [`Overflow`] policy.
//!
//! ## Feature flags
//!
//! Enable at least one input backend: `input-native` (auto-selects), or
//! `input-v4l` / `input-avfoundation` / `input-msmf` for a specific platform.
//! `input-gstreamer` is cross-platform and additionally handles
//! `rtsp://` / `http://` / `file://` URLs via `CameraIndex::String`.
//!
//! - `mjpeg` (default): MJPEG decoding via `mozjpeg`.
//! - `runner`: threaded helper [`CameraRunner`].
//! - `output-wgpu`: direct frame-to-wgpu texture copy.
//! - `serialize`: serde on core types.
//! - `logging`: route internal diagnostics through the `log` crate.
//! - `device-test`: build the integration tests in `tests/device_tests.rs`.
//!   Requires a real camera; CI runners without one should leave it off.
//!
//! ## Traits
//!
//! Backends implement [`CameraDevice`](nokhwa_core::traits::CameraDevice) plus
//! any of [`FrameSource`](nokhwa_core::traits::FrameSource),
//! [`ShutterCapture`](nokhwa_core::traits::ShutterCapture),
//! [`EventSource`](nokhwa_core::traits::EventSource).

// Ensure at least one input backend is enabled (skip during docs-only builds).
#[cfg(not(feature = "docs-only"))]
#[cfg(not(any(
    feature = "input-avfoundation",
    feature = "input-v4l",
    feature = "input-msmf",
    feature = "input-gstreamer"
)))]
compile_error!(
    "nokhwa requires at least one input-* feature to be enabled \
     (e.g. input-native / input-auto, input-avfoundation, input-v4l, input-msmf, \
     input-gstreamer)"
);

/// Raw access to each of Nokhwa's backends.
pub mod backends;
mod init;
mod query;

// Layer 2: `open()` / `OpenedCamera` and the per-capability wrappers.
// Layer 3 (CameraRunner) lives in the `runner` module below, gated on the
// `runner` feature.
pub mod session;
pub use session::{open, HybridCamera, OpenRequest, OpenedCamera, ShutterCamera, StreamCamera};

#[cfg(feature = "runner")]
pub mod runner;
#[cfg(feature = "runner")]
pub use runner::{CameraRunner, Overflow, RunnerConfig};

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
