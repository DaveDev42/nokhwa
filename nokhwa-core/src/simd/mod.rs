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

//! SIMD-optimized pixel format conversion routines.
//!
//! Provides accelerated pixel format conversion using platform intrinsics:
//! - **BGR-to-RGB**: NEON on aarch64, SSSE3/AVX2 on `x86_64`, scalar fallback
//! - **YUYV-to-RGB/RGBA**: NEON on aarch64, SSE4.1 on `x86_64`, scalar fallback
//! - **NV12-to-RGB/RGBA**: NEON on aarch64, SSE4.1 on `x86_64`, scalar fallback
//! - **RGB/BGR-to-RGBA**: NEON on aarch64, SSSE3 on `x86_64`, scalar fallback
//! - **YUYV Y-extraction**: NEON on aarch64, SSSE3 on `x86_64`, scalar fallback
//! - **RGB-to-Luma**: NEON on aarch64, SSE2 on `x86_64`, scalar fallback

mod bgr_to_rgb;
mod nv12_to_rgb;
mod rgb_to_luma;
mod rgb_to_rgba;
mod yuyv_extract_luma;
mod yuyv_to_rgb;

pub(crate) use bgr_to_rgb::bgr_to_rgb_simd;
pub(crate) use nv12_to_rgb::nv12_to_rgb_simd;
pub(crate) use rgb_to_luma::rgb_to_luma_simd;
pub(crate) use rgb_to_rgba::{bgr_to_rgba_simd, rgb_to_rgba_simd};
pub(crate) use yuyv_extract_luma::yuyv_extract_luma_simd;
pub(crate) use yuyv_to_rgb::{yuyv_to_rgb_simd, yuyv_to_rgba_simd};

/// Public re-exports of SIMD and scalar pixel conversion routines for benchmarking.
///
/// Gated behind the `bench` feature. Not part of the stable API.
#[cfg(feature = "bench")]
pub mod bench_exports {
    pub use super::bgr_to_rgb::{bgr_to_rgb_scalar, bgr_to_rgb_simd};
    pub use super::nv12_to_rgb::{nv12_to_rgb_scalar, nv12_to_rgb_simd};
    pub use super::rgb_to_luma::{rgb_to_luma_scalar, rgb_to_luma_simd};
    pub use super::yuyv_extract_luma::{yuyv_extract_luma_scalar, yuyv_extract_luma_simd};
    pub use super::yuyv_to_rgb::{
        yuyv_to_rgb_scalar, yuyv_to_rgb_simd, yuyv_to_rgba_scalar, yuyv_to_rgba_simd,
    };
}
