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
//! - **BGR-to-RGB**: NEON on aarch64, SSSE3 on `x86_64`, scalar fallback
//! - **YUYV-to-RGB/RGBA**: NEON on aarch64, scalar fallback on other architectures

use crate::types::yuyv444_to_rgb;
#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::{int16x8_t, int32x4_t, uint8x8_t};

// ──────────────────────────────────────────────
// BGR → RGB  (3-byte channel swap)
// ──────────────────────────────────────────────

/// Swap BGR to RGB using SIMD where available.
/// `src` and `dst` must be the same length and a multiple of 3.
#[inline]
pub fn bgr_to_rgb_simd(src: &[u8], dst: &mut [u8]) {
    debug_assert_eq!(src.len(), dst.len());
    debug_assert!(src.len().is_multiple_of(3));

    #[cfg(target_arch = "aarch64")]
    bgr_to_rgb_neon(src, dst);

    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is always available on x86_64
    unsafe {
        bgr_to_rgb_sse2(src, dst);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    bgr_to_rgb_scalar(src, dst);
}

/// Scalar BGR-to-RGB fallback.
#[inline]
fn bgr_to_rgb_scalar(src: &[u8], dst: &mut [u8]) {
    for (src_px, dst_px) in src.chunks_exact(3).zip(dst.chunks_exact_mut(3)) {
        dst_px[0] = src_px[2];
        dst_px[1] = src_px[1];
        dst_px[2] = src_px[0];
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn bgr_to_rgb_neon(src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::{uint8x8x3_t, vld3_u8, vst3_u8};

    let len = src.len();
    // vld3_u8 loads 8x3 = 24 bytes (8 BGR pixels)
    let simd_end = len - (len % 24);
    let mut idx = 0;

    while idx < simd_end {
        // SAFETY: we checked bounds; pointers are valid for 24-byte read/write
        unsafe {
            let bgr = vld3_u8(src.as_ptr().add(idx));
            // bgr.0 = B, bgr.1 = G, bgr.2 = R — swap 0 and 2
            let rgb = uint8x8x3_t(bgr.2, bgr.1, bgr.0);
            vst3_u8(dst.as_mut_ptr().add(idx), rgb);
        }
        idx += 24;
    }

    // Handle remaining pixels with scalar
    bgr_to_rgb_scalar(&src[simd_end..], &mut dst[simd_end..]);
}

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn bgr_to_rgb_sse2(src: &[u8], dst: &mut [u8]) {
    // SSSE3 (pshufb) is widely available on all x86_64 CPUs since 2006.
    if is_x86_feature_detected!("ssse3") {
        bgr_to_rgb_ssse3(src, dst);
        return;
    }

    // Pure SSE2 fallback — scalar, since SSE2 shuffle is awkward for 3-byte stride
    bgr_to_rgb_scalar(src, dst);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn bgr_to_rgb_ssse3(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::{_mm_loadu_si128, _mm_setr_epi8, _mm_shuffle_epi8, _mm_storeu_si128};

    let len = src.len();

    // Shuffle mask: swap bytes within each 3-byte group for 5 pixels (15 bytes)
    let shuffle = _mm_setr_epi8(2, 1, 0, 5, 4, 3, 8, 7, 6, 11, 10, 9, 14, 13, 12, -1);

    let simd_end = len - (len % 15);
    let mut idx = 0;

    while idx < simd_end {
        // Load 16 bytes (we only use 15)
        if idx + 16 <= len {
            let vec = _mm_loadu_si128(src.as_ptr().add(idx).cast());
            let shuffled = _mm_shuffle_epi8(vec, shuffle);
            // Store only 15 bytes — can't use _mm_storeu for partial; copy manually
            let mut tmp = [0u8; 16];
            _mm_storeu_si128(tmp.as_mut_ptr().cast(), shuffled);
            dst[idx..idx + 15].copy_from_slice(&tmp[..15]);
        } else {
            bgr_to_rgb_scalar(&src[idx..], &mut dst[idx..]);
            return;
        }
        idx += 15;
    }

    bgr_to_rgb_scalar(&src[simd_end..], &mut dst[simd_end..]);
}

// ──────────────────────────────────────────────
// YUYV → RGB  (YUV 4:2:2 conversion)
// ──────────────────────────────────────────────

/// Convert a YUYV chunk to RGB using SIMD where available.
/// Processes `src` (YUYV 4:2:2, 4 bytes per 2 pixels) into `dst` (RGB888, 3 bytes per pixel).
/// `dst` must be `(src.len() / 4) * 6` bytes.
#[inline]
pub fn yuyv_to_rgb_simd(src: &[u8], dst: &mut [u8]) {
    debug_assert!(src.len().is_multiple_of(4));
    debug_assert_eq!(dst.len(), (src.len() / 4) * 6);

    #[cfg(target_arch = "aarch64")]
    yuyv_to_rgb_neon(src, dst);

    #[cfg(not(target_arch = "aarch64"))]
    yuyv_to_rgb_scalar(src, dst);
}

/// Convert YUYV to RGBA using SIMD where available.
/// `dst` must be `(src.len() / 4) * 8` bytes.
#[inline]
pub fn yuyv_to_rgba_simd(src: &[u8], dst: &mut [u8]) {
    debug_assert!(src.len().is_multiple_of(4));
    debug_assert_eq!(dst.len(), (src.len() / 4) * 8);

    #[cfg(target_arch = "aarch64")]
    yuyv_to_rgba_neon(src, dst);

    #[cfg(not(target_arch = "aarch64"))]
    yuyv_to_rgba_scalar(src, dst);
}

/// Scalar YUYV-to-RGB fallback.
#[inline]
fn yuyv_to_rgb_scalar(src: &[u8], dst: &mut [u8]) {
    for (chunk, out) in src.chunks_exact(4).zip(dst.chunks_exact_mut(6)) {
        let luma0 = i32::from(chunk[0]);
        let cb = i32::from(chunk[1]);
        let luma1 = i32::from(chunk[2]);
        let cr = i32::from(chunk[3]);

        let px0 = yuyv444_to_rgb(luma0, cb, cr);
        let px1 = yuyv444_to_rgb(luma1, cb, cr);

        out[0] = px0[0];
        out[1] = px0[1];
        out[2] = px0[2];
        out[3] = px1[0];
        out[4] = px1[1];
        out[5] = px1[2];
    }
}

/// Scalar YUYV-to-RGBA fallback.
#[inline]
fn yuyv_to_rgba_scalar(src: &[u8], dst: &mut [u8]) {
    for (chunk, out) in src.chunks_exact(4).zip(dst.chunks_exact_mut(8)) {
        let luma0 = i32::from(chunk[0]);
        let cb = i32::from(chunk[1]);
        let luma1 = i32::from(chunk[2]);
        let cr = i32::from(chunk[3]);

        let px0 = yuyv444_to_rgb(luma0, cb, cr);
        let px1 = yuyv444_to_rgb(luma1, cb, cr);

        out[0] = px0[0];
        out[1] = px0[1];
        out[2] = px0[2];
        out[3] = 255;
        out[4] = px1[0];
        out[5] = px1[1];
        out[6] = px1[2];
        out[7] = 255;
    }
}

// ──────────────────────────────────────────────
// aarch64 NEON YUYV → RGB
// ──────────────────────────────────────────────

#[cfg(target_arch = "aarch64")]
#[inline]
fn yuyv_to_rgb_neon(src: &[u8], dst: &mut [u8]) {
    // Process 8 YUYV pairs (32 bytes -> 48 bytes RGB, 16 pixels) per iteration
    let yuyv_chunk = 32; // 8 YUYV pairs
    let rgb_chunk = 48; // 16 RGB pixels
    let simd_end = src.len() - (src.len() % yuyv_chunk);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        // SAFETY: bounds checked above, intrinsics require valid pointers
        unsafe {
            yuyv_8pair_to_rgb_neon(src.as_ptr().add(si), dst.as_mut_ptr().add(di));
        }
        si += yuyv_chunk;
        di += rgb_chunk;
    }

    // Scalar tail
    yuyv_to_rgb_scalar(&src[si..], &mut dst[di..]);
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn yuyv_to_rgba_neon(src: &[u8], dst: &mut [u8]) {
    let yuyv_chunk = 32;
    let rgba_chunk = 64; // 16 RGBA pixels
    let simd_end = src.len() - (src.len() % yuyv_chunk);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        unsafe {
            yuyv_8pair_to_rgba_neon(src.as_ptr().add(si), dst.as_mut_ptr().add(di));
        }
        si += yuyv_chunk;
        di += rgba_chunk;
    }

    yuyv_to_rgba_scalar(&src[si..], &mut dst[di..]);
}

/// Process 8 YUYV pairs (32 bytes) into 16 RGB pixels (48 bytes) using NEON.
///
/// Each YUYV pair is `[Y0, U, Y1, V]` (4 bytes producing 2 RGB pixels).
/// We process 8 pairs = 16 pixels using i32 vector math.
#[cfg(target_arch = "aarch64")]
#[allow(clippy::similar_names)] // y0/y1, u/v suffixed names are standard YUV terminology
#[inline]
unsafe fn yuyv_8pair_to_rgb_neon(src: *const u8, dst: *mut u8) {
    use std::arch::aarch64::{
        uint8x16x3_t, vcombine_u8, vdupq_n_s16, vdupq_n_s32, vld4_u8, vmovl_u8,
        vreinterpretq_s16_u16, vst3q_u8, vsubq_s16, vzip1_u8, vzip2_u8,
    };

    // Load 32 bytes as 4x8-byte interleaved: Y0, U, Y1, V
    let yuyv = vld4_u8(src);

    // Widen u8 to i16
    let y0_wide = vreinterpretq_s16_u16(vmovl_u8(yuyv.0)); // 8x Y0 as i16
    let y1_wide = vreinterpretq_s16_u16(vmovl_u8(yuyv.2)); // 8x Y1 as i16
    let cb_wide = vreinterpretq_s16_u16(vmovl_u8(yuyv.1)); // 8x U as i16
    let cr_wide = vreinterpretq_s16_u16(vmovl_u8(yuyv.3)); // 8x V as i16

    let offset16 = vdupq_n_s16(16);
    let offset128 = vdupq_n_s16(128);
    let bias = vdupq_n_s32(128);

    // d = U - 128, e = V - 128
    let cb_centered = vsubq_s16(cb_wide, offset128);
    let cr_centered = vsubq_s16(cr_wide, offset128);

    // Process Y0 pixels (first 8) and Y1 pixels (second 8)
    let rgb0 = yuv_to_rgb_neon_8px(vsubq_s16(y0_wide, offset16), cb_centered, cr_centered, bias);
    let rgb1 = yuv_to_rgb_neon_8px(vsubq_s16(y1_wide, offset16), cb_centered, cr_centered, bias);

    // Interleave: Y0[i] and Y1[i] share the same U/V, output as pixel pairs
    let r_lo = vzip1_u8(rgb0.0, rgb1.0);
    let r_hi = vzip2_u8(rgb0.0, rgb1.0);
    let g_lo = vzip1_u8(rgb0.1, rgb1.1);
    let g_hi = vzip2_u8(rgb0.1, rgb1.1);
    let b_lo = vzip1_u8(rgb0.2, rgb1.2);
    let b_hi = vzip2_u8(rgb0.2, rgb1.2);

    // Combine lo+hi into 16-byte vectors
    let r_all = vcombine_u8(r_lo, r_hi);
    let g_all = vcombine_u8(g_lo, g_hi);
    let b_all = vcombine_u8(b_lo, b_hi);

    // Store as interleaved RGB
    vst3q_u8(dst, uint8x16x3_t(r_all, g_all, b_all));
}

/// Process 8 YUYV pairs (32 bytes) into 16 RGBA pixels (64 bytes) using NEON.
#[cfg(target_arch = "aarch64")]
#[allow(clippy::similar_names)]
#[inline]
unsafe fn yuyv_8pair_to_rgba_neon(src: *const u8, dst: *mut u8) {
    use std::arch::aarch64::{
        uint8x16x4_t, vcombine_u8, vdupq_n_s16, vdupq_n_s32, vdupq_n_u8, vld4_u8, vmovl_u8,
        vreinterpretq_s16_u16, vst4q_u8, vsubq_s16, vzip1_u8, vzip2_u8,
    };

    let yuyv = vld4_u8(src);

    let y0_wide = vreinterpretq_s16_u16(vmovl_u8(yuyv.0));
    let y1_wide = vreinterpretq_s16_u16(vmovl_u8(yuyv.2));
    let cb_wide = vreinterpretq_s16_u16(vmovl_u8(yuyv.1));
    let cr_wide = vreinterpretq_s16_u16(vmovl_u8(yuyv.3));

    let offset16 = vdupq_n_s16(16);
    let offset128 = vdupq_n_s16(128);
    let bias = vdupq_n_s32(128);

    let cb_centered = vsubq_s16(cb_wide, offset128);
    let cr_centered = vsubq_s16(cr_wide, offset128);

    let rgb0 = yuv_to_rgb_neon_8px(vsubq_s16(y0_wide, offset16), cb_centered, cr_centered, bias);
    let rgb1 = yuv_to_rgb_neon_8px(vsubq_s16(y1_wide, offset16), cb_centered, cr_centered, bias);

    let r_lo = vzip1_u8(rgb0.0, rgb1.0);
    let r_hi = vzip2_u8(rgb0.0, rgb1.0);
    let g_lo = vzip1_u8(rgb0.1, rgb1.1);
    let g_hi = vzip2_u8(rgb0.1, rgb1.1);
    let b_lo = vzip1_u8(rgb0.2, rgb1.2);
    let b_hi = vzip2_u8(rgb0.2, rgb1.2);

    let r_all = vcombine_u8(r_lo, r_hi);
    let g_all = vcombine_u8(g_lo, g_hi);
    let b_all = vcombine_u8(b_lo, b_hi);
    let a_all = vdupq_n_u8(255);

    vst4q_u8(dst, uint8x16x4_t(r_all, g_all, b_all, a_all));
}

/// Compute 8 RGB pixels from 8 Y values and shared U/V (all as i16 vectors).
/// Returns (R, G, B) as `uint8x8_t` (clamped to `[0, 255]`).
///
/// Formula (fixed-point, matching `yuyv444_to_rgb`):
/// - `c298y = y_minus16 * 298`
/// - `R = (c298y + 409*cr_centered + 128) >> 8`
/// - `G = (c298y - 100*cb_centered - 208*cr_centered + 128) >> 8`
/// - `B = (c298y + 516*cb_centered + 128) >> 8`
#[cfg(target_arch = "aarch64")]
#[allow(clippy::similar_names)]
#[inline]
unsafe fn yuv_to_rgb_neon_8px(
    y_minus16: int16x8_t,
    cb_centered: int16x8_t,
    cr_centered: int16x8_t,
    bias: int32x4_t,
) -> (uint8x8_t, uint8x8_t, uint8x8_t) {
    use std::arch::aarch64::{
        vaddq_s32, vcombine_s16, vdupq_n_s16, vdupq_n_s32, vget_high_s16, vget_low_s16, vmaxq_s16,
        vmovl_s16, vmulq_s32, vqmovn_s32, vqmovun_s16, vshrq_n_s32, vsubq_s32,
    };

    // c298y = y_minus16 * 298 — can overflow i16 (max 239*298 = 71222), so use i32
    let y_lo = vmovl_s16(vget_low_s16(y_minus16)); // 4x i32
    let y_hi = vmovl_s16(vget_high_s16(y_minus16));
    let cb_lo = vmovl_s16(vget_low_s16(cb_centered));
    let cb_hi = vmovl_s16(vget_high_s16(cb_centered));
    let cr_lo = vmovl_s16(vget_low_s16(cr_centered));
    let cr_hi = vmovl_s16(vget_high_s16(cr_centered));

    let k298 = vdupq_n_s32(298);
    let k409 = vdupq_n_s32(409);
    let k100 = vdupq_n_s32(100);
    let k208 = vdupq_n_s32(208);
    let k516 = vdupq_n_s32(516);

    let c298y_lo = vmulq_s32(y_lo, k298);
    let c298y_hi = vmulq_s32(y_hi, k298);

    // R = (c298y + 409*e + 128) >> 8
    let red_lo = vshrq_n_s32::<8>(vaddq_s32(vaddq_s32(c298y_lo, vmulq_s32(cr_lo, k409)), bias));
    let red_hi = vshrq_n_s32::<8>(vaddq_s32(vaddq_s32(c298y_hi, vmulq_s32(cr_hi, k409)), bias));

    // G = (c298y - 100*d - 208*e + 128) >> 8
    let grn_lo = vshrq_n_s32::<8>(vaddq_s32(
        vsubq_s32(
            vsubq_s32(c298y_lo, vmulq_s32(cb_lo, k100)),
            vmulq_s32(cr_lo, k208),
        ),
        bias,
    ));
    let grn_hi = vshrq_n_s32::<8>(vaddq_s32(
        vsubq_s32(
            vsubq_s32(c298y_hi, vmulq_s32(cb_hi, k100)),
            vmulq_s32(cr_hi, k208),
        ),
        bias,
    ));

    // B = (c298y + 516*d + 128) >> 8
    let blu_lo = vshrq_n_s32::<8>(vaddq_s32(vaddq_s32(c298y_lo, vmulq_s32(cb_lo, k516)), bias));
    let blu_hi = vshrq_n_s32::<8>(vaddq_s32(vaddq_s32(c298y_hi, vmulq_s32(cb_hi, k516)), bias));

    // Narrow i32 -> i16 with saturating narrowing (vqmovn_s32). Post-shift values
    // fit in i16 for valid YUV input (max ~481, min ~-122), but we use saturating
    // narrowing for defense-in-depth against any future formula changes.
    let red_16 = vcombine_s16(vqmovn_s32(red_lo), vqmovn_s32(red_hi));
    let grn_16 = vcombine_s16(vqmovn_s32(grn_lo), vqmovn_s32(grn_hi));
    let blu_16 = vcombine_s16(vqmovn_s32(blu_lo), vqmovn_s32(blu_hi));

    // max(0, val) then saturating narrow to u8
    let zero = vdupq_n_s16(0);
    (
        vqmovun_s16(vmaxq_s16(red_16, zero)),
        vqmovun_s16(vmaxq_s16(grn_16, zero)),
        vqmovun_s16(vmaxq_s16(blu_16, zero)),
    )
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgr_to_rgb_roundtrip() {
        let bgr = vec![10u8, 20, 30, 40, 50, 60, 70, 80, 90];
        let mut rgb = vec![0u8; 9];
        bgr_to_rgb_simd(&bgr, &mut rgb);
        assert_eq!(rgb, vec![30, 20, 10, 60, 50, 40, 90, 80, 70]);

        // Double-swap should recover original
        let mut recovered = vec![0u8; 9];
        bgr_to_rgb_simd(&rgb, &mut recovered);
        assert_eq!(recovered, bgr);
    }

    #[test]
    fn bgr_to_rgb_large_buffer() {
        // Test with a buffer large enough to exercise SIMD paths (>24 bytes for NEON)
        let pixel_count = 100;
        let bgr: Vec<u8> = (0..pixel_count * 3).map(|i| (i % 256) as u8).collect();
        let mut rgb = vec![0u8; bgr.len()];
        bgr_to_rgb_simd(&bgr, &mut rgb);

        for i in 0..pixel_count {
            let si = i * 3;
            assert_eq!(rgb[si], bgr[si + 2], "R mismatch at pixel {i}");
            assert_eq!(rgb[si + 1], bgr[si + 1], "G mismatch at pixel {i}");
            assert_eq!(rgb[si + 2], bgr[si], "B mismatch at pixel {i}");
        }
    }

    #[test]
    fn yuyv_to_rgb_matches_scalar() {
        let yuyv: Vec<u8> = vec![
            128, 128, 128, 128, // neutral gray
            16, 128, 16, 128, // near black
            235, 128, 235, 128, // near white
            180, 50, 100, 200, // mixed values
        ];

        let mut simd_out = vec![0u8; 24];
        let mut scalar_out = vec![0u8; 24];

        yuyv_to_rgb_simd(&yuyv, &mut simd_out);
        yuyv_to_rgb_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "SIMD and scalar YUYV-to-RGB must match"
        );
    }

    #[test]
    fn yuyv_to_rgb_large_matches_scalar() {
        // Large enough to exercise NEON path (>32 bytes = 8 YUYV pairs)
        let pair_count = 64;
        let yuyv: Vec<u8> = (0..pair_count * 4).map(|i| (i * 7 % 256) as u8).collect();
        let mut simd_out = vec![0u8; pair_count * 6];
        let mut scalar_out = vec![0u8; pair_count * 6];

        yuyv_to_rgb_simd(&yuyv, &mut simd_out);
        yuyv_to_rgb_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "SIMD and scalar YUYV-to-RGB must match for large buffer"
        );
    }

    #[test]
    fn yuyv_to_rgba_matches_scalar() {
        let yuyv: Vec<u8> = vec![
            128, 128, 128, 128, 16, 128, 16, 128, 235, 128, 235, 128, 180, 50, 100, 200,
        ];

        let mut simd_out = vec![0u8; 32];
        let mut scalar_out = vec![0u8; 32];

        yuyv_to_rgba_simd(&yuyv, &mut simd_out);
        yuyv_to_rgba_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "SIMD and scalar YUYV-to-RGBA must match"
        );
    }

    #[test]
    fn yuyv_to_rgba_large_matches_scalar() {
        let pair_count = 64;
        let yuyv: Vec<u8> = (0..pair_count * 4).map(|i| (i * 7 % 256) as u8).collect();
        let mut simd_out = vec![0u8; pair_count * 8];
        let mut scalar_out = vec![0u8; pair_count * 8];

        yuyv_to_rgba_simd(&yuyv, &mut simd_out);
        yuyv_to_rgba_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "SIMD and scalar YUYV-to-RGBA must match for large buffer"
        );
    }

    #[test]
    fn yuyv_neutral_gray() {
        let yuyv = vec![128u8, 128, 128, 128];
        let mut rgb = vec![0u8; 6];
        yuyv_to_rgb_simd(&yuyv, &mut rgb);

        let expected = yuyv444_to_rgb(128, 128, 128);
        assert_eq!(rgb[0], expected[0]);
        assert_eq!(rgb[1], expected[1]);
        assert_eq!(rgb[2], expected[2]);
    }

    #[test]
    fn bgr_to_rgb_empty() {
        let bgr: Vec<u8> = vec![];
        let mut rgb: Vec<u8> = vec![];
        bgr_to_rgb_simd(&bgr, &mut rgb);
        assert!(rgb.is_empty());
    }

    #[test]
    fn yuyv_to_rgb_empty() {
        let yuyv: Vec<u8> = vec![];
        let mut rgb: Vec<u8> = vec![];
        yuyv_to_rgb_simd(&yuyv, &mut rgb);
        assert!(rgb.is_empty());
    }

    #[test]
    fn yuyv_to_rgba_empty() {
        let yuyv: Vec<u8> = vec![];
        let mut rgba: Vec<u8> = vec![];
        yuyv_to_rgba_simd(&yuyv, &mut rgba);
        assert!(rgba.is_empty());
    }

    #[test]
    fn yuyv_to_rgb_non_aligned_exercises_tail() {
        // 13 YUYV pairs: not a multiple of 8, so on NEON the first 8 pairs go
        // through the vectorized path and the remaining 5 through scalar tail.
        let pair_count = 13;
        let yuyv: Vec<u8> = (0..pair_count * 4).map(|i| (i * 11 % 256) as u8).collect();
        let mut simd_out = vec![0u8; pair_count * 6];
        let mut scalar_out = vec![0u8; pair_count * 6];

        yuyv_to_rgb_simd(&yuyv, &mut simd_out);
        yuyv_to_rgb_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "SIMD and scalar must match for non-8-aligned pair count"
        );
    }

    #[test]
    fn yuyv_to_rgba_non_aligned_exercises_tail() {
        let pair_count = 13;
        let yuyv: Vec<u8> = (0..pair_count * 4).map(|i| (i * 11 % 256) as u8).collect();
        let mut simd_out = vec![0u8; pair_count * 8];
        let mut scalar_out = vec![0u8; pair_count * 8];

        yuyv_to_rgba_simd(&yuyv, &mut simd_out);
        yuyv_to_rgba_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "SIMD and scalar RGBA must match for non-8-aligned pair count"
        );
    }

    #[test]
    fn yuyv_to_rgb_extreme_values() {
        // Extreme YUV values that push clamping boundaries hard
        let yuyv: Vec<u8> = vec![
            0, 0, 0, 255, // Y=0,U=0,Y=0,V=255: deep negative green, strong red
            255, 255, 255, 0, // Y=255,U=255,Y=255,V=0: strong blue, negative red
            0, 0, 0, 0, // Y=0,U=0,Y=0,V=0: all minimum
            255, 255, 255, 255, // Y=255,U=255,Y=255,V=255: all maximum
        ];

        let mut simd_out = vec![0u8; 24];
        let mut scalar_out = vec![0u8; 24];

        yuyv_to_rgb_simd(&yuyv, &mut simd_out);
        yuyv_to_rgb_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "SIMD and scalar must match for extreme YUV values"
        );
    }
}
