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

/// Internal bench-only wrappers. See `crate::bench_exports` for the public path.
///
/// These forward to the `pub(crate)` SIMD/scalar routines so that the internal
/// API surface is not broadened by the `bench` feature. `#[doc(hidden)]` and
/// not covered by semver.
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_exports {
    /// # Panics
    /// If `src.len() != dst.len()` or `src.len() % 3 != 0`.
    pub fn bgr_to_rgb_simd(src: &[u8], dst: &mut [u8]) {
        super::bgr_to_rgb::bgr_to_rgb_simd(src, dst);
    }
    /// # Panics
    /// If `src.len() != dst.len()` or `src.len() % 3 != 0`.
    pub fn bgr_to_rgb_scalar(src: &[u8], dst: &mut [u8]) {
        super::bgr_to_rgb::bgr_to_rgb_scalar(src, dst);
    }

    /// # Panics
    /// If `src.len() % 4 != 0` or `dst.len() != (src.len() / 4) * 6`.
    pub fn yuyv_to_rgb_simd(src: &[u8], dst: &mut [u8]) {
        super::yuyv_to_rgb::yuyv_to_rgb_simd(src, dst);
    }
    /// # Panics
    /// If `src.len() % 4 != 0` or `dst.len() != (src.len() / 4) * 6`.
    pub fn yuyv_to_rgb_scalar(src: &[u8], dst: &mut [u8]) {
        super::yuyv_to_rgb::yuyv_to_rgb_scalar(src, dst);
    }
    /// # Panics
    /// If `src.len() % 4 != 0` or `dst.len() != (src.len() / 4) * 8`.
    pub fn yuyv_to_rgba_simd(src: &[u8], dst: &mut [u8]) {
        super::yuyv_to_rgb::yuyv_to_rgba_simd(src, dst);
    }
    /// # Panics
    /// If `src.len() % 4 != 0` or `dst.len() != (src.len() / 4) * 8`.
    pub fn yuyv_to_rgba_scalar(src: &[u8], dst: &mut [u8]) {
        super::yuyv_to_rgb::yuyv_to_rgba_scalar(src, dst);
    }

    /// # Panics
    /// If `width`/`height` are not even, or buffer sizes do not match NV12 layout.
    pub fn nv12_to_rgb_simd(width: usize, height: usize, data: &[u8], out: &mut [u8]) {
        super::nv12_to_rgb::nv12_to_rgb_simd(width, height, data, out, false);
    }
    /// # Panics
    /// If `width`/`height` are not even, or buffer sizes do not match NV12 layout.
    pub fn nv12_to_rgb_scalar(width: usize, height: usize, data: &[u8], out: &mut [u8]) {
        super::nv12_to_rgb::nv12_to_rgb_scalar(width, height, data, out, false);
    }
    /// # Panics
    /// If `width`/`height` are not even, or buffer sizes do not match NV12 layout.
    pub fn nv12_to_rgba_simd(width: usize, height: usize, data: &[u8], out: &mut [u8]) {
        super::nv12_to_rgb::nv12_to_rgb_simd(width, height, data, out, true);
    }
    /// # Panics
    /// If `width`/`height` are not even, or buffer sizes do not match NV12 layout.
    pub fn nv12_to_rgba_scalar(width: usize, height: usize, data: &[u8], out: &mut [u8]) {
        super::nv12_to_rgb::nv12_to_rgb_scalar(width, height, data, out, true);
    }

    /// # Panics
    /// If `src.len() % 3 != 0` or `dst.len() != src.len() / 3`.
    pub fn rgb_to_luma_simd(src: &[u8], dst: &mut [u8]) {
        super::rgb_to_luma::rgb_to_luma_simd(src, dst);
    }
    /// # Panics
    /// If `src.len() % 3 != 0` or `dst.len() != src.len() / 3`.
    pub fn rgb_to_luma_scalar(src: &[u8], dst: &mut [u8]) {
        super::rgb_to_luma::rgb_to_luma_scalar(src, dst);
    }

    /// # Panics
    /// If `src.len() % 4 != 0` or `dst.len() != src.len() / 2`.
    pub fn yuyv_extract_luma_simd(src: &[u8], dst: &mut [u8]) {
        super::yuyv_extract_luma::yuyv_extract_luma_simd(src, dst);
    }
    /// # Panics
    /// If `src.len() % 4 != 0` or `dst.len() != src.len() / 2`.
    pub fn yuyv_extract_luma_scalar(src: &[u8], dst: &mut [u8]) {
        super::yuyv_extract_luma::yuyv_extract_luma_scalar(src, dst);
    }
}
