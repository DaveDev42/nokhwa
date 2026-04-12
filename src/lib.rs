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
//! This crate is mid-migration to the 0.13.0 trait-split architecture.
//! The top-level `Camera` / `CallbackCamera` API has been removed and will
//! be superseded by `CameraSession` (Layer 2) and `CameraRunner` (Layer 3)
//! in subsequent tasks. See TODO.md for migration status.

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
// Layer 3 (CameraRunner) arrives in later tasks (T14-T15).
pub mod session;
pub use session::{
    CameraSession, HybridCamera, OpenRequest, OpenedCamera, ShutterCamera, StreamCamera,
};

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
