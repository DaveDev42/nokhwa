#![deny(clippy::pedantic)]
#![warn(clippy::all)]
#![cfg_attr(feature = "test-fail-warnings", deny(warnings))]
#![cfg_attr(feature = "docs-features", feature(doc_cfg))]
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

//! Core type definitions for `nokhwa`.
//!
//! This crate holds the platform-agnostic building blocks used by the top-level
//! [`nokhwa`](https://docs.rs/nokhwa) crate and its backend bindings. It has no
//! platform-specific code: everything here compiles everywhere.
//!
//! # Key types
//!
//! ## Frames and buffers
//!
//! - [`buffer::Buffer`] — raw frame payload with resolution, source
//!   [`types::FrameFormat`], and a capture [`buffer::TimestampKind`].
//! - [`frame::Frame<F>`] — a type-safe wrapper around [`buffer::Buffer`] tagged
//!   with a compile-time [`format_types::CaptureFormat`]. Cheap to clone
//!   (backed by reference-counted bytes).
//!
//! ## Capture format markers
//!
//! Zero-sized types in [`format_types`] that implement
//! [`format_types::CaptureFormat`]:
//!
//! - [`format_types::Yuyv`] — YUYV 4:2:2 packed
//! - [`format_types::Nv12`] — NV12 (YUV 4:2:0 bi-planar)
//! - [`format_types::Mjpeg`] — Motion-JPEG compressed
//! - [`format_types::Gray`] — 8-bit grayscale
//! - [`format_types::RawRgb`] — raw RGB888
//! - [`format_types::RawBgr`] — raw BGR888
//!
//! Each marker pins a [`frame::Frame<F>`] to one wire format, and the
//! conversion traits below are only implemented for formats where the
//! conversion makes sense.
//!
//! ## Lazy conversion traits
//!
//! Conversions are two-step: call `into_rgb()` / `into_rgba()` /
//! `into_luma()` on a [`frame::Frame<F>`] to get a lightweight conversion
//! struct, then call `materialize()` (or `write_to()`) to actually run the
//! decoder and produce an [`image::ImageBuffer`].
//!
//! - [`frame::IntoRgb`] → [`frame::RgbConversion`] → `ImageBuffer<Rgb<u8>, Vec<u8>>`
//! - [`frame::IntoRgba`] → [`frame::RgbaConversion`] → `ImageBuffer<Rgba<u8>, Vec<u8>>`
//! - [`frame::IntoLuma`] → [`frame::LumaConversion`] → `ImageBuffer<Luma<u8>, Vec<u8>>`
//!
//! `Frame<Gray>` intentionally does not implement [`frame::IntoRgb`] or
//! [`frame::IntoRgba`]: grayscale frames carry no color information, so the
//! compiler rejects the conversion instead of silently upsampling. `Gray`
//! does implement [`frame::IntoLuma`], so `frame.into_luma().materialize()`
//! is the positive path for grayscale.
//!
//! ## Traits and error types
//!
//! - [`traits::CaptureBackendTrait`] — the platform-backend contract used by
//!   `nokhwa::Camera`.
//! - [`error::NokhwaError`] — unified error type for the whole workspace.
pub mod buffer;
pub mod error;
pub mod format_types;
pub mod frame;
#[cfg(not(feature = "bench"))]
pub(crate) mod simd;
#[cfg(feature = "bench")]
pub mod simd;
pub mod traits;
pub mod types;
