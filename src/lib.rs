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

//! # nokhwa
//! A Simple-to-use, cross-platform Rust Webcam Capture Library
//!
//! The raw backends can be found in [`backends`](crate::backends)
//!
//! The [`Camera`] struct is what you will likely use.
//!
//! The recommended default feature to enable is `input-native` (also available as `input-auto`).
//! The library will not work without at least one `input-*` feature enabled.
//!
//! Please read the README.md for more.

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
     (e.g. input-native, input-avfoundation, input-v4l, input-msmf, input-opencv)"
);

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
