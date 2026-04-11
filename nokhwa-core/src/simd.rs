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

#[cfg(test)]
use crate::types::yuyv444_to_rgb;
#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::{int16x8_t, int32x4_t, uint8x8_t};

// ──────────────────────────────────────────────
// BGR → RGB  (3-byte channel swap)
// ──────────────────────────────────────────────

/// Swap BGR to RGB using SIMD where available.
/// `src` and `dst` must be the same length and a multiple of 3.
#[inline]
pub(crate) fn bgr_to_rgb_simd(src: &[u8], dst: &mut [u8]) {
    assert_eq!(src.len(), dst.len());
    assert!(src.len().is_multiple_of(3));

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
    if is_x86_feature_detected!("avx2") {
        bgr_to_rgb_avx2(src, dst);
        return;
    }

    // SSSE3 (pshufb) is widely available on all x86_64 CPUs since 2006.
    if is_x86_feature_detected!("ssse3") {
        bgr_to_rgb_ssse3(src, dst);
        return;
    }

    // Pure SSE2 fallback — scalar, since SSE2 shuffle is awkward for 3-byte stride
    bgr_to_rgb_scalar(src, dst);
}

/// AVX2 BGR→RGB: processes 30 bytes (10 pixels) per iteration.
///
/// `vpshufb` operates independently within each 128-bit lane, so we must
/// load two 128-bit chunks at correct 15-byte offsets (pixel boundaries)
/// and combine them into a 256-bit register. A single 256-bit load would
/// misalign lane 1 since 15 is not a power of 2.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn bgr_to_rgb_avx2(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::{
        _mm256_set_m128i, _mm256_setr_epi8, _mm256_shuffle_epi8, _mm256_storeu_si256,
        _mm_loadu_si128,
    };

    let len = src.len();

    // Same 15-byte swap pattern in both 128-bit lanes
    let shuffle = _mm256_setr_epi8(
        2, 1, 0, 5, 4, 3, 8, 7, 6, 11, 10, 9, 14, 13, 12, -1, // lane 0
        2, 1, 0, 5, 4, 3, 8, 7, 6, 11, 10, 9, 14, 13, 12, -1, // lane 1
    );

    // Each iteration processes 30 bytes (10 BGR pixels → 10 RGB pixels).
    // We need idx + 15 + 16 <= len for the second 128-bit load to be safe.
    let simd_limit = len.saturating_sub(30);
    let mut idx = 0;

    while idx < simd_limit {
        // SAFETY: idx + 31 <= len, both 128-bit loads are in bounds.
        // Load two 128-bit halves at pixel-aligned offsets (0 and 15 bytes apart).
        let lo = _mm_loadu_si128(src.as_ptr().add(idx).cast());
        let hi = _mm_loadu_si128(src.as_ptr().add(idx + 15).cast());
        let combined = _mm256_set_m128i(hi, lo);
        let shuffled = _mm256_shuffle_epi8(combined, shuffle);

        let mut tmp = [0u8; 32];
        _mm256_storeu_si256(tmp.as_mut_ptr().cast(), shuffled);
        // Lane 0 (tmp[0..15]): 5 swapped pixels from src[idx..idx+15]
        // Lane 1 (tmp[16..31]): 5 swapped pixels from src[idx+15..idx+30]
        dst[idx..idx + 15].copy_from_slice(&tmp[..15]);
        dst[idx + 15..idx + 30].copy_from_slice(&tmp[16..31]);
        idx += 30;
    }

    // Fall through to SSSE3 for the remainder (which itself has a scalar tail)
    bgr_to_rgb_ssse3(&src[idx..], &mut dst[idx..]);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn bgr_to_rgb_ssse3(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::{_mm_loadu_si128, _mm_setr_epi8, _mm_shuffle_epi8, _mm_storeu_si128};

    let len = src.len();

    // Shuffle mask: swap bytes within each 3-byte group for 5 pixels (15 bytes)
    let shuffle = _mm_setr_epi8(2, 1, 0, 5, 4, 3, 8, 7, 6, 11, 10, 9, 14, 13, 12, -1);

    // Each iteration loads 16 bytes but only uses 15, so we need at least 16
    // readable bytes from the start of every chunk. Compute the last safe
    // start index: we can process a chunk starting at idx when idx + 16 <= len.
    let simd_limit = len.saturating_sub(15);
    let mut idx = 0;

    while idx < simd_limit {
        let vec = _mm_loadu_si128(src.as_ptr().add(idx).cast());
        let shuffled = _mm_shuffle_epi8(vec, shuffle);
        // Store only 15 bytes — can't use _mm_storeu for partial; copy manually
        let mut tmp = [0u8; 16];
        _mm_storeu_si128(tmp.as_mut_ptr().cast(), shuffled);
        dst[idx..idx + 15].copy_from_slice(&tmp[..15]);
        idx += 15;
    }

    bgr_to_rgb_scalar(&src[idx..], &mut dst[idx..]);
}

// ──────────────────────────────────────────────
// YUYV → RGB  (YUV 4:2:2 conversion)
// ──────────────────────────────────────────────

/// Convert a YUYV chunk to RGB using SIMD where available.
/// Processes `src` (YUYV 4:2:2, 4 bytes per 2 pixels) into `dst` (RGB888, 3 bytes per pixel).
/// `dst` must be `(src.len() / 4) * 6` bytes.
#[inline]
pub(crate) fn yuyv_to_rgb_simd(src: &[u8], dst: &mut [u8]) {
    assert!(src.len().is_multiple_of(4));
    assert_eq!(dst.len(), (src.len() / 4) * 6);

    #[cfg(target_arch = "aarch64")]
    yuyv_to_rgb_neon(src, dst);

    #[cfg(target_arch = "x86_64")]
    // SAFETY: runtime feature detection gates the SSE4.1 path
    unsafe {
        yuyv_to_rgb_x86(src, dst);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    yuyv_to_rgb_scalar(src, dst);
}

/// Convert YUYV to RGBA using SIMD where available.
/// `dst` must be `(src.len() / 4) * 8` bytes.
#[inline]
pub(crate) fn yuyv_to_rgba_simd(src: &[u8], dst: &mut [u8]) {
    assert!(src.len().is_multiple_of(4));
    assert_eq!(dst.len(), (src.len() / 4) * 8);

    #[cfg(target_arch = "aarch64")]
    yuyv_to_rgba_neon(src, dst);

    #[cfg(target_arch = "x86_64")]
    // SAFETY: runtime feature detection gates the SSE4.1 path
    unsafe {
        yuyv_to_rgba_x86(src, dst);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    yuyv_to_rgba_scalar(src, dst);
}

/// Scalar YUYV-to-RGB fallback.
///
/// # Safety invariants (upheld by caller `yuyv_to_rgb_simd`):
///   - `src.len()` is a multiple of 4 (asserted).
///   - `dst.len() == (src.len() / 4) * 6` (asserted).
///
/// Let N = `src.len()` / 4 (number of YUYV pairs).
///   - Source: each iteration reads `si..si+3` where si = i*4, max si+3 = (N-1)*4+3 = `src.len()-1` ✓
///   - Dest: each iteration writes `di..di+5` where di = i*6, max di+5 = (N-1)*6+5 = `dst.len()-1` ✓
#[inline]
#[allow(clippy::cast_sign_loss)]
fn yuyv_to_rgb_scalar(src: &[u8], dst: &mut [u8]) {
    debug_assert!(src.len().is_multiple_of(4));
    debug_assert_eq!(dst.len(), (src.len() / 4) * 6);

    let n = src.len() / 4;
    let mut si = 0;
    let mut di = 0;

    for _ in 0..n {
        // SAFETY: si+3 <= src.len()-1 because si = i*4 and i < N = src.len()/4 (see proof above).
        let luma0 = unsafe { i32::from(*src.get_unchecked(si)) };
        let cb = unsafe { i32::from(*src.get_unchecked(si + 1)) };
        let luma1 = unsafe { i32::from(*src.get_unchecked(si + 2)) };
        let cr = unsafe { i32::from(*src.get_unchecked(si + 3)) };

        // Inline BT.601 YUV→RGB: avoids intermediate [u8; 3] arrays.
        let d = cb - 128;
        let e = cr - 128;

        let c298_0 = (luma0 - 16) * 298;
        let r0 = ((c298_0 + 409 * e + 128) >> 8).clamp(0, 255) as u8;
        let g0 = ((c298_0 - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u8;
        let b0 = ((c298_0 + 516 * d + 128) >> 8).clamp(0, 255) as u8;

        let c298_1 = (luma1 - 16) * 298;
        let r1 = ((c298_1 + 409 * e + 128) >> 8).clamp(0, 255) as u8;
        let g1 = ((c298_1 - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u8;
        let b1 = ((c298_1 + 516 * d + 128) >> 8).clamp(0, 255) as u8;

        // SAFETY: di+5 <= dst.len()-1 because di = i*6 and i < N, so di+5 = i*6+5 ≤ (N-1)*6+5 = N*6-1 = dst.len()-1 (see proof above).
        unsafe {
            *dst.get_unchecked_mut(di) = r0;
            *dst.get_unchecked_mut(di + 1) = g0;
            *dst.get_unchecked_mut(di + 2) = b0;
            *dst.get_unchecked_mut(di + 3) = r1;
            *dst.get_unchecked_mut(di + 4) = g1;
            *dst.get_unchecked_mut(di + 5) = b1;
        }

        si += 4;
        di += 6;
    }
}

/// Scalar YUYV-to-RGBA fallback.
///
/// # Safety invariants (upheld by caller `yuyv_to_rgba_simd`):
///   - `src.len()` is a multiple of 4 (asserted).
///   - `dst.len() == (src.len() / 4) * 8` (asserted).
///
/// Let N = `src.len()` / 4 (number of YUYV pairs).
///   - Source: each iteration reads `si..si+3` where si = i*4, max si+3 = (N-1)*4+3 = `src.len()-1` ✓
///   - Dest: each iteration writes `di..di+7` where di = i*8, max di+7 = (N-1)*8+7 = `dst.len()-1` ✓
#[inline]
#[allow(clippy::cast_sign_loss)]
fn yuyv_to_rgba_scalar(src: &[u8], dst: &mut [u8]) {
    debug_assert!(src.len().is_multiple_of(4));
    debug_assert_eq!(dst.len(), (src.len() / 4) * 8);

    let n = src.len() / 4;
    let mut si = 0;
    let mut di = 0;

    for _ in 0..n {
        // SAFETY: si+3 <= src.len()-1 because si = i*4 and i < N = src.len()/4 (see proof above).
        let luma0 = unsafe { i32::from(*src.get_unchecked(si)) };
        let cb = unsafe { i32::from(*src.get_unchecked(si + 1)) };
        let luma1 = unsafe { i32::from(*src.get_unchecked(si + 2)) };
        let cr = unsafe { i32::from(*src.get_unchecked(si + 3)) };

        // Inline BT.601 YUV→RGB: avoids intermediate [u8; 4] arrays.
        let d = cb - 128;
        let e = cr - 128;

        let c298_0 = (luma0 - 16) * 298;
        let r0 = ((c298_0 + 409 * e + 128) >> 8).clamp(0, 255) as u8;
        let g0 = ((c298_0 - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u8;
        let b0 = ((c298_0 + 516 * d + 128) >> 8).clamp(0, 255) as u8;

        let c298_1 = (luma1 - 16) * 298;
        let r1 = ((c298_1 + 409 * e + 128) >> 8).clamp(0, 255) as u8;
        let g1 = ((c298_1 - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u8;
        let b1 = ((c298_1 + 516 * d + 128) >> 8).clamp(0, 255) as u8;

        // SAFETY: di+7 <= dst.len()-1 because di = i*8 and i < N, so di+7 = i*8+7 ≤ (N-1)*8+7 = N*8-1 = dst.len()-1 (see proof above).
        unsafe {
            *dst.get_unchecked_mut(di) = r0;
            *dst.get_unchecked_mut(di + 1) = g0;
            *dst.get_unchecked_mut(di + 2) = b0;
            *dst.get_unchecked_mut(di + 3) = 255;
            *dst.get_unchecked_mut(di + 4) = r1;
            *dst.get_unchecked_mut(di + 5) = g1;
            *dst.get_unchecked_mut(di + 6) = b1;
            *dst.get_unchecked_mut(di + 7) = 255;
        }

        si += 4;
        di += 8;
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
// x86_64 SSE4.1 YUYV → RGB / RGBA
// ──────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn yuyv_to_rgb_x86(src: &[u8], dst: &mut [u8]) {
    if is_x86_feature_detected!("sse4.1") {
        yuyv_to_rgb_sse41(src, dst);
    } else {
        yuyv_to_rgb_scalar(src, dst);
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn yuyv_to_rgba_x86(src: &[u8], dst: &mut [u8]) {
    if is_x86_feature_detected!("sse4.1") {
        yuyv_to_rgba_sse41(src, dst);
    } else {
        yuyv_to_rgba_scalar(src, dst);
    }
}

/// Process YUYV→RGB using SSE4.1. Handles 8 pixels (4 YUYV pairs = 16 bytes) per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
#[allow(clippy::similar_names, clippy::cast_sign_loss)]
unsafe fn yuyv_to_rgb_sse41(src: &[u8], dst: &mut [u8]) {
    // 4 YUYV pairs = 16 src bytes → 8 RGB pixels = 24 dst bytes
    let simd_end = src.len() - (src.len() % 16);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        yuyv_8px_to_rgb_sse41(src.as_ptr().add(si), dst.as_mut_ptr().add(di));
        si += 16;
        di += 24;
    }

    // Scalar tail
    yuyv_to_rgb_scalar(&src[si..], &mut dst[di..]);
}

/// Process YUYV→RGBA using SSE4.1.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
#[allow(clippy::similar_names, clippy::cast_sign_loss)]
unsafe fn yuyv_to_rgba_sse41(src: &[u8], dst: &mut [u8]) {
    let simd_end = src.len() - (src.len() % 16);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        yuyv_8px_to_rgba_sse41(src.as_ptr().add(si), dst.as_mut_ptr().add(di));
        si += 16;
        di += 32;
    }

    yuyv_to_rgba_scalar(&src[si..], &mut dst[di..]);
}

/// Convert 4 YUYV pairs (16 bytes, 8 pixels) to 24 RGB bytes using SSE4.1.
///
/// YUYV layout: [Y0,U0,Y1,V0, Y2,U1,Y3,V1, Y4,U2,Y5,V2, Y6,U3,Y7,V3]
/// We extract Y, U, V channels, duplicate U/V for paired pixels, then apply BT.601.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
#[allow(clippy::similar_names)]
unsafe fn yuyv_8px_to_rgb_sse41(src: *const u8, dst: *mut u8) {
    use std::arch::x86_64::{
        _mm_add_epi32, _mm_cvtepi16_epi32, _mm_loadl_epi64, _mm_loadu_si128, _mm_max_epi16,
        _mm_mullo_epi32, _mm_packs_epi32, _mm_packus_epi16, _mm_set1_epi16, _mm_set1_epi32,
        _mm_setr_epi8, _mm_setzero_si128, _mm_shuffle_epi8, _mm_srai_epi32, _mm_srli_si128,
        _mm_storeu_si128, _mm_sub_epi16, _mm_sub_epi32, _mm_unpacklo_epi8,
    };

    let zero = _mm_setzero_si128();

    // Load 16 bytes of YUYV data
    let yuyv = _mm_loadu_si128(src.cast());

    // Extract channels using pshufb:
    // Y: bytes 0,2,4,6,8,10,12,14
    let y_shuf = _mm_setr_epi8(0, 2, 4, 6, 8, 10, 12, 14, -1, -1, -1, -1, -1, -1, -1, -1);
    // U: bytes 1,1,5,5,9,9,13,13 (duplicated for paired pixels)
    let u_shuf = _mm_setr_epi8(1, 1, 5, 5, 9, 9, 13, 13, -1, -1, -1, -1, -1, -1, -1, -1);
    // V: bytes 3,3,7,7,11,11,15,15 (duplicated)
    let v_shuf = _mm_setr_epi8(3, 3, 7, 7, 11, 11, 15, 15, -1, -1, -1, -1, -1, -1, -1, -1);

    let y8 = _mm_shuffle_epi8(yuyv, y_shuf);
    let u8_dup = _mm_shuffle_epi8(yuyv, u_shuf);
    let v8_dup = _mm_shuffle_epi8(yuyv, v_shuf);

    let offset16 = _mm_set1_epi16(16);
    let offset128 = _mm_set1_epi16(128);
    let bias32 = _mm_set1_epi32(128);
    let k298 = _mm_set1_epi32(298);
    let k409 = _mm_set1_epi32(409);
    let k100 = _mm_set1_epi32(100);
    let k208 = _mm_set1_epi32(208);
    let k516 = _mm_set1_epi32(516);

    // Widen to i16 and apply offsets
    let y16 = _mm_sub_epi16(_mm_unpacklo_epi8(y8, zero), offset16);
    let u16 = _mm_sub_epi16(_mm_unpacklo_epi8(u8_dup, zero), offset128);
    let v16 = _mm_sub_epi16(_mm_unpacklo_epi8(v8_dup, zero), offset128);

    // Process low 4 pixels (i16 → i32)
    let y_lo = _mm_cvtepi16_epi32(y16);
    let u_lo = _mm_cvtepi16_epi32(u16);
    let v_lo = _mm_cvtepi16_epi32(v16);

    let c298y_lo = _mm_mullo_epi32(y_lo, k298);
    let r_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_add_epi32(c298y_lo, _mm_mullo_epi32(v_lo, k409)),
        bias32,
    ));
    let g_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_sub_epi32(
            _mm_sub_epi32(c298y_lo, _mm_mullo_epi32(u_lo, k100)),
            _mm_mullo_epi32(v_lo, k208),
        ),
        bias32,
    ));
    let b_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_add_epi32(c298y_lo, _mm_mullo_epi32(u_lo, k516)),
        bias32,
    ));

    // Process high 4 pixels
    let y_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(y16));
    let u_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(u16));
    let v_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(v16));

    let c298y_hi = _mm_mullo_epi32(y_hi, k298);
    let r_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_add_epi32(c298y_hi, _mm_mullo_epi32(v_hi, k409)),
        bias32,
    ));
    let g_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_sub_epi32(
            _mm_sub_epi32(c298y_hi, _mm_mullo_epi32(u_hi, k100)),
            _mm_mullo_epi32(v_hi, k208),
        ),
        bias32,
    ));
    let b_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_add_epi32(c298y_hi, _mm_mullo_epi32(u_hi, k516)),
        bias32,
    ));

    // Pack and clamp
    let r16 = _mm_max_epi16(_mm_packs_epi32(r_lo, r_hi), zero);
    let g16 = _mm_max_epi16(_mm_packs_epi32(g_lo, g_hi), zero);
    let b16 = _mm_max_epi16(_mm_packs_epi32(b_lo, b_hi), zero);
    let r8 = _mm_packus_epi16(r16, zero);
    let g8 = _mm_packus_epi16(g16, zero);
    let b8 = _mm_packus_epi16(b16, zero);

    // Interleave R,G,B into output (extract to arrays, write pixel-by-pixel)
    let mut r_arr = [0u8; 16];
    let mut g_arr = [0u8; 16];
    let mut b_arr = [0u8; 16];
    _mm_storeu_si128(r_arr.as_mut_ptr().cast(), r8);
    _mm_storeu_si128(g_arr.as_mut_ptr().cast(), g8);
    _mm_storeu_si128(b_arr.as_mut_ptr().cast(), b8);

    for i in 0..8 {
        *dst.add(i * 3) = r_arr[i];
        *dst.add(i * 3 + 1) = g_arr[i];
        *dst.add(i * 3 + 2) = b_arr[i];
    }
}

/// Convert 4 YUYV pairs (16 bytes, 8 pixels) to 32 RGBA bytes using SSE4.1.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
#[allow(clippy::similar_names)]
unsafe fn yuyv_8px_to_rgba_sse41(src: *const u8, dst: *mut u8) {
    use std::arch::x86_64::{
        _mm_add_epi32, _mm_cvtepi16_epi32, _mm_loadu_si128, _mm_max_epi16, _mm_mullo_epi32,
        _mm_packs_epi32, _mm_packus_epi16, _mm_set1_epi16, _mm_set1_epi32, _mm_setr_epi8,
        _mm_setzero_si128, _mm_shuffle_epi8, _mm_srai_epi32, _mm_srli_si128, _mm_storeu_si128,
        _mm_sub_epi16, _mm_sub_epi32, _mm_unpacklo_epi8,
    };

    let zero = _mm_setzero_si128();
    let yuyv = _mm_loadu_si128(src.cast());

    let y_shuf = _mm_setr_epi8(0, 2, 4, 6, 8, 10, 12, 14, -1, -1, -1, -1, -1, -1, -1, -1);
    let u_shuf = _mm_setr_epi8(1, 1, 5, 5, 9, 9, 13, 13, -1, -1, -1, -1, -1, -1, -1, -1);
    let v_shuf = _mm_setr_epi8(3, 3, 7, 7, 11, 11, 15, 15, -1, -1, -1, -1, -1, -1, -1, -1);

    let y8 = _mm_shuffle_epi8(yuyv, y_shuf);
    let u8_dup = _mm_shuffle_epi8(yuyv, u_shuf);
    let v8_dup = _mm_shuffle_epi8(yuyv, v_shuf);

    let offset16 = _mm_set1_epi16(16);
    let offset128 = _mm_set1_epi16(128);
    let bias32 = _mm_set1_epi32(128);
    let k298 = _mm_set1_epi32(298);
    let k409 = _mm_set1_epi32(409);
    let k100 = _mm_set1_epi32(100);
    let k208 = _mm_set1_epi32(208);
    let k516 = _mm_set1_epi32(516);

    let y16 = _mm_sub_epi16(_mm_unpacklo_epi8(y8, zero), offset16);
    let u16 = _mm_sub_epi16(_mm_unpacklo_epi8(u8_dup, zero), offset128);
    let v16 = _mm_sub_epi16(_mm_unpacklo_epi8(v8_dup, zero), offset128);

    let y_lo = _mm_cvtepi16_epi32(y16);
    let u_lo = _mm_cvtepi16_epi32(u16);
    let v_lo = _mm_cvtepi16_epi32(v16);

    let c298y_lo = _mm_mullo_epi32(y_lo, k298);
    let r_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_add_epi32(c298y_lo, _mm_mullo_epi32(v_lo, k409)),
        bias32,
    ));
    let g_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_sub_epi32(
            _mm_sub_epi32(c298y_lo, _mm_mullo_epi32(u_lo, k100)),
            _mm_mullo_epi32(v_lo, k208),
        ),
        bias32,
    ));
    let b_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_add_epi32(c298y_lo, _mm_mullo_epi32(u_lo, k516)),
        bias32,
    ));

    let y_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(y16));
    let u_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(u16));
    let v_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(v16));

    let c298y_hi = _mm_mullo_epi32(y_hi, k298);
    let r_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_add_epi32(c298y_hi, _mm_mullo_epi32(v_hi, k409)),
        bias32,
    ));
    let g_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_sub_epi32(
            _mm_sub_epi32(c298y_hi, _mm_mullo_epi32(u_hi, k100)),
            _mm_mullo_epi32(v_hi, k208),
        ),
        bias32,
    ));
    let b_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
        _mm_add_epi32(c298y_hi, _mm_mullo_epi32(u_hi, k516)),
        bias32,
    ));

    let r16 = _mm_max_epi16(_mm_packs_epi32(r_lo, r_hi), zero);
    let g16 = _mm_max_epi16(_mm_packs_epi32(g_lo, g_hi), zero);
    let b16 = _mm_max_epi16(_mm_packs_epi32(b_lo, b_hi), zero);
    let r8 = _mm_packus_epi16(r16, zero);
    let g8 = _mm_packus_epi16(g16, zero);
    let b8 = _mm_packus_epi16(b16, zero);

    let mut r_arr = [0u8; 16];
    let mut g_arr = [0u8; 16];
    let mut b_arr = [0u8; 16];
    _mm_storeu_si128(r_arr.as_mut_ptr().cast(), r8);
    _mm_storeu_si128(g_arr.as_mut_ptr().cast(), g8);
    _mm_storeu_si128(b_arr.as_mut_ptr().cast(), b8);

    for i in 0..8 {
        *dst.add(i * 4) = r_arr[i];
        *dst.add(i * 4 + 1) = g_arr[i];
        *dst.add(i * 4 + 2) = b_arr[i];
        *dst.add(i * 4 + 3) = 255;
    }
}

// ──────────────────────────────────────────────
// NV12 → RGB / RGBA  (YUV 4:2:0 bi-planar)
// ──────────────────────────────────────────────

/// Convert NV12 data to RGB/RGBA using SIMD where available.
///
/// NV12 layout: full Y plane (`width * height` bytes) followed by an interleaved
/// UV plane (`width * height / 2` bytes, with U/V pairs for each 2×2 block).
///
/// `width` and `height` must be even. `out` must be `width * height * pxsize` bytes
/// where `pxsize` is 3 (RGB) or 4 (RGBA).
#[inline]
pub(crate) fn nv12_to_rgb_simd(
    width: usize,
    height: usize,
    data: &[u8],
    out: &mut [u8],
    rgba: bool,
) {
    #[cfg(target_arch = "aarch64")]
    nv12_to_rgb_neon(width, height, data, out, rgba);

    #[cfg(target_arch = "x86_64")]
    // SAFETY: runtime feature detection gates the SSE4.1 path
    unsafe {
        nv12_to_rgb_x86(width, height, data, out, rgba);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    nv12_to_rgb_scalar(width, height, data, out, rgba);
}

/// Scalar NV12→RGB/RGBA fallback.
#[allow(
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    dead_code
)]
fn nv12_to_rgb_scalar(width: usize, height: usize, data: &[u8], out: &mut [u8], rgba: bool) {
    let pxsize = if rgba { 4 } else { 3 };
    let y_plane = &data[..width * height];
    let uv_plane = &data[width * height..];

    for row in 0..height {
        let y_row = &y_plane[row * width..][..width];
        let uv_row = &uv_plane[(row / 2) * width..][..width];
        let out_row = &mut out[row * width * pxsize..][..width * pxsize];

        for col in (0..width).step_by(2) {
            let cb = i32::from(uv_row[col]);
            let cr = i32::from(uv_row[col + 1]);
            let cb_centered = cb - 128;
            let cr_centered = cr - 128;

            for off in 0..2 {
                let luma = i32::from(y_row[col + off]);
                let c298 = (luma - 16) * 298;
                let red = ((c298 + 409 * cr_centered + 128) >> 8).clamp(0, 255) as u8;
                let grn =
                    ((c298 - 100 * cb_centered - 208 * cr_centered + 128) >> 8).clamp(0, 255) as u8;
                let blu = ((c298 + 516 * cb_centered + 128) >> 8).clamp(0, 255) as u8;

                let oi = (col + off) * pxsize;
                out_row[oi] = red;
                out_row[oi + 1] = grn;
                out_row[oi + 2] = blu;
                if rgba {
                    out_row[oi + 3] = 255;
                }
            }
        }
    }
}

#[cfg(target_arch = "aarch64")]
fn nv12_to_rgb_neon(width: usize, height: usize, data: &[u8], out: &mut [u8], rgba: bool) {
    let pxsize = if rgba { 4 } else { 3 };
    let y_plane = &data[..width * height];
    let uv_plane = &data[width * height..];
    // Process 16 Y pixels (+ 8 UV pairs) per SIMD iteration
    let simd_width = width - (width % 16);

    for row in 0..height {
        let y_row = &y_plane[row * width..];
        let uv_row = &uv_plane[(row / 2) * width..];
        let out_row = &mut out[row * width * pxsize..];

        let mut col = 0;
        while col < simd_width {
            // SAFETY: col + 16 <= width (simd_width is aligned down), pointers valid
            unsafe {
                nv12_16px_to_rgb_neon(
                    y_row.as_ptr().add(col),
                    uv_row.as_ptr().add(col),
                    out_row.as_mut_ptr().add(col * pxsize),
                    rgba,
                );
            }
            col += 16;
        }

        // Scalar tail for remaining pixels
        nv12_scalar_tail(y_row, uv_row, out_row, col, width, pxsize, rgba);
    }
}

/// Process remaining pixels in a row using scalar code.
/// Shared by NEON and SSE tails to avoid duplicating the BT.601 math.
#[allow(clippy::cast_sign_loss, clippy::similar_names)]
fn nv12_scalar_tail(
    y_row: &[u8],
    uv_row: &[u8],
    out_row: &mut [u8],
    start_col: usize,
    width: usize,
    pxsize: usize,
    rgba: bool,
) {
    let mut col = start_col;
    while col < width {
        let uv_col = col & !1;
        let cb = i32::from(uv_row[uv_col]);
        let cr = i32::from(uv_row[uv_col + 1]);
        let cb_centered = cb - 128;
        let cr_centered = cr - 128;
        let luma = i32::from(y_row[col]);
        let c298 = (luma - 16) * 298;
        let red = ((c298 + 409 * cr_centered + 128) >> 8).clamp(0, 255) as u8;
        let grn = ((c298 - 100 * cb_centered - 208 * cr_centered + 128) >> 8).clamp(0, 255) as u8;
        let blu = ((c298 + 516 * cb_centered + 128) >> 8).clamp(0, 255) as u8;
        let oi = col * pxsize;
        out_row[oi] = red;
        out_row[oi + 1] = grn;
        out_row[oi + 2] = blu;
        if rgba {
            out_row[oi + 3] = 255;
        }
        col += 1;
    }
}

/// Process 16 Y pixels + 8 UV pairs into 16 RGB or RGBA pixels using NEON.
///
/// Each UV pair covers 2 horizontal Y pixels, so 8 UV pairs serve 16 Y pixels.
/// UV values are duplicated to match: `[u0,u1,u2,u3]` → `[u0,u0,u1,u1,u2,u2,u3,u3]`.
#[cfg(target_arch = "aarch64")]
#[allow(clippy::similar_names)]
unsafe fn nv12_16px_to_rgb_neon(y_ptr: *const u8, uv_ptr: *const u8, out_ptr: *mut u8, rgba: bool) {
    use std::arch::aarch64::{
        uint8x16x3_t, uint8x16x4_t, vcombine_s16, vcombine_u8, vdupq_n_s16, vdupq_n_s32,
        vdupq_n_u8, vget_high_s16, vget_high_u8, vget_low_s16, vget_low_u8, vld1q_u8, vld2_u8,
        vmovl_u8, vreinterpretq_s16_u16, vst3q_u8, vst4q_u8, vsubq_s16, vzip1q_s16,
    };

    // Load 16 Y values
    let y_vals = vld1q_u8(y_ptr);
    let y_lo = vget_low_u8(y_vals);
    let y_hi = vget_high_u8(y_vals);

    // Load 8 UV pairs (16 bytes: U0,V0,U1,V1,...) → deinterleave to 8xU, 8xV
    let uv = vld2_u8(uv_ptr);

    let offset16 = vdupq_n_s16(16);
    let offset128 = vdupq_n_s16(128);
    let bias = vdupq_n_s32(128);

    // Widen U,V to i16 and center around 0
    let u_wide = vsubq_s16(vreinterpretq_s16_u16(vmovl_u8(uv.0)), offset128); // 8x i16
    let v_wide = vsubq_s16(vreinterpretq_s16_u16(vmovl_u8(uv.1)), offset128);

    // Widen Y to i16 and subtract 16
    let y0_wide = vsubq_s16(vreinterpretq_s16_u16(vmovl_u8(y_lo)), offset16);
    let y1_wide = vsubq_s16(vreinterpretq_s16_u16(vmovl_u8(y_hi)), offset16);

    // Duplicate each UV for 2 horizontal pixels:
    // [u0,u1,u2,u3] → [u0,u0,u1,u1,u2,u2,u3,u3]
    // vzip1q_s16(a, a) interleaves the low halves element-by-element.
    let u_lo_half = vget_low_s16(u_wide);
    let u_hi_half = vget_high_s16(u_wide);
    let v_lo_half = vget_low_s16(v_wide);
    let v_hi_half = vget_high_s16(v_wide);

    let u_dup_lo = vcombine_s16(u_lo_half, u_lo_half);
    let u_for_y0 = vzip1q_s16(u_dup_lo, u_dup_lo);
    let u_dup_hi = vcombine_s16(u_hi_half, u_hi_half);
    let u_for_y1 = vzip1q_s16(u_dup_hi, u_dup_hi);

    let v_dup_lo = vcombine_s16(v_lo_half, v_lo_half);
    let v_for_y0 = vzip1q_s16(v_dup_lo, v_dup_lo);
    let v_dup_hi = vcombine_s16(v_hi_half, v_hi_half);
    let v_for_y1 = vzip1q_s16(v_dup_hi, v_dup_hi);

    let rgb0 = yuv_to_rgb_neon_8px(y0_wide, u_for_y0, v_for_y0, bias);
    let rgb1 = yuv_to_rgb_neon_8px(y1_wide, u_for_y1, v_for_y1, bias);

    let r_all = vcombine_u8(rgb0.0, rgb1.0);
    let g_all = vcombine_u8(rgb0.1, rgb1.1);
    let b_all = vcombine_u8(rgb0.2, rgb1.2);

    if rgba {
        let a_all = vdupq_n_u8(255);
        vst4q_u8(out_ptr, uint8x16x4_t(r_all, g_all, b_all, a_all));
    } else {
        vst3q_u8(out_ptr, uint8x16x3_t(r_all, g_all, b_all));
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn nv12_to_rgb_x86(width: usize, height: usize, data: &[u8], out: &mut [u8], rgba: bool) {
    if is_x86_feature_detected!("sse4.1") {
        nv12_to_rgb_sse41(width, height, data, out, rgba);
    } else {
        nv12_to_rgb_scalar(width, height, data, out, rgba);
    }
}

/// Process NV12 using SSE4.1 (available on all x86_64 CPUs since ~2008).
/// Processes 8 Y pixels + 4 UV pairs per iteration using `_mm_mullo_epi32` (SSE4.1).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
#[allow(clippy::similar_names, clippy::cast_sign_loss, clippy::too_many_lines)]
unsafe fn nv12_to_rgb_sse41(width: usize, height: usize, data: &[u8], out: &mut [u8], rgba: bool) {
    use std::arch::x86_64::{
        _mm_add_epi32, _mm_loadl_epi64, _mm_max_epi16, _mm_mullo_epi32, _mm_packs_epi32,
        _mm_packus_epi16, _mm_set1_epi16, _mm_set1_epi32, _mm_setr_epi8, _mm_setzero_si128,
        _mm_shuffle_epi8, _mm_srai_epi32, _mm_storeu_si128, _mm_sub_epi16, _mm_sub_epi32,
        _mm_unpacklo_epi8,
    };

    let pxsize = if rgba { 4 } else { 3 };
    let y_plane = &data[..width * height];
    let uv_plane = &data[width * height..];
    let simd_width = width - (width % 8);

    // Shuffle masks for duplicating UV values (SSSE3 pshufb)
    let u_shuf = _mm_setr_epi8(0, 0, 2, 2, 4, 4, 6, 6, -1, -1, -1, -1, -1, -1, -1, -1);
    let v_shuf = _mm_setr_epi8(1, 1, 3, 3, 5, 5, 7, 7, -1, -1, -1, -1, -1, -1, -1, -1);

    let zero = _mm_setzero_si128();
    let offset16 = _mm_set1_epi16(16);
    let offset128 = _mm_set1_epi16(128);
    let bias32 = _mm_set1_epi32(128);
    let k298 = _mm_set1_epi32(298);
    let k409 = _mm_set1_epi32(409);
    let k100 = _mm_set1_epi32(100);
    let k208 = _mm_set1_epi32(208);
    let k516 = _mm_set1_epi32(516);

    for row in 0..height {
        let y_row_ptr = y_plane.as_ptr().add(row * width);
        let uv_row_ptr = uv_plane.as_ptr().add((row / 2) * width);
        let out_row = &mut out[row * width * pxsize..];

        let mut col = 0;
        while col < simd_width {
            // SAFETY: col + 8 <= width, pointers are valid
            // Load 8 Y values (64-bit load, zero-extended)
            let y8 = _mm_loadl_epi64(y_row_ptr.add(col).cast());
            let y16 = _mm_sub_epi16(_mm_unpacklo_epi8(y8, zero), offset16);

            // Load 8 bytes of UV (4 UV pairs) and duplicate each U,V for 2 pixels
            let uv8 = _mm_loadl_epi64(uv_row_ptr.add(col).cast());
            let u8_dup = _mm_shuffle_epi8(uv8, u_shuf);
            let v8_dup = _mm_shuffle_epi8(uv8, v_shuf);

            let u16 = _mm_sub_epi16(_mm_unpacklo_epi8(u8_dup, zero), offset128);
            let v16 = _mm_sub_epi16(_mm_unpacklo_epi8(v8_dup, zero), offset128);

            // Sign-extend low 4 i16 → i32 using SSE4.1 _mm_cvtepi16_epi32
            use std::arch::x86_64::_mm_cvtepi16_epi32;
            let y_lo = _mm_cvtepi16_epi32(y16);
            let u_lo = _mm_cvtepi16_epi32(u16);
            let v_lo = _mm_cvtepi16_epi32(v16);

            let c298y_lo = _mm_mullo_epi32(y_lo, k298);
            let r_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
                _mm_add_epi32(c298y_lo, _mm_mullo_epi32(v_lo, k409)),
                bias32,
            ));
            let g_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
                _mm_sub_epi32(
                    _mm_sub_epi32(c298y_lo, _mm_mullo_epi32(u_lo, k100)),
                    _mm_mullo_epi32(v_lo, k208),
                ),
                bias32,
            ));
            let b_lo = _mm_srai_epi32::<8>(_mm_add_epi32(
                _mm_add_epi32(c298y_lo, _mm_mullo_epi32(u_lo, k516)),
                bias32,
            ));

            // High 4 pixels: shift the i16x8 right by 4 lanes, then sign-extend
            use std::arch::x86_64::_mm_srli_si128;
            let y_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(y16));
            let u_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(u16));
            let v_hi = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(v16));

            let c298y_hi = _mm_mullo_epi32(y_hi, k298);
            let r_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
                _mm_add_epi32(c298y_hi, _mm_mullo_epi32(v_hi, k409)),
                bias32,
            ));
            let g_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
                _mm_sub_epi32(
                    _mm_sub_epi32(c298y_hi, _mm_mullo_epi32(u_hi, k100)),
                    _mm_mullo_epi32(v_hi, k208),
                ),
                bias32,
            ));
            let b_hi = _mm_srai_epi32::<8>(_mm_add_epi32(
                _mm_add_epi32(c298y_hi, _mm_mullo_epi32(u_hi, k516)),
                bias32,
            ));

            // Pack i32 → i16 (saturating), clamp ≥ 0, then i16 → u8 (unsigned saturating)
            let r16 = _mm_max_epi16(_mm_packs_epi32(r_lo, r_hi), zero);
            let g16 = _mm_max_epi16(_mm_packs_epi32(g_lo, g_hi), zero);
            let b16 = _mm_max_epi16(_mm_packs_epi32(b_lo, b_hi), zero);

            let r8 = _mm_packus_epi16(r16, zero);
            let g8 = _mm_packus_epi16(g16, zero);
            let b8 = _mm_packus_epi16(b16, zero);

            // Extract to arrays and interleave into output
            let mut r_arr = [0u8; 16];
            let mut g_arr = [0u8; 16];
            let mut b_arr = [0u8; 16];
            _mm_storeu_si128(r_arr.as_mut_ptr().cast(), r8);
            _mm_storeu_si128(g_arr.as_mut_ptr().cast(), g8);
            _mm_storeu_si128(b_arr.as_mut_ptr().cast(), b8);

            let out_base = &mut out_row[col * pxsize..];
            for i in 0..8 {
                let oi = i * pxsize;
                out_base[oi] = r_arr[i];
                out_base[oi + 1] = g_arr[i];
                out_base[oi + 2] = b_arr[i];
                if rgba {
                    out_base[oi + 3] = 255;
                }
            }

            col += 8;
        }

        // Scalar tail
        let y_row = &y_plane[row * width..];
        let uv_row = &uv_plane[(row / 2) * width..];
        nv12_scalar_tail(y_row, uv_row, out_row, col, width, pxsize, rgba);
    }
}

// ──────────────────────────────────────────────
// RGB → RGBA / BGR → RGBA  (3-byte to 4-byte expansion)
// ──────────────────────────────────────────────

/// Expand RGB888 to RGBA8888 with alpha=255 using SIMD where available.
/// `src.len()` must be a multiple of 3 and `dst.len()` must be `(src.len() / 3) * 4`.
#[inline]
pub(crate) fn rgb_to_rgba_simd(src: &[u8], dst: &mut [u8]) {
    assert!(src.len().is_multiple_of(3));
    assert_eq!(dst.len(), (src.len() / 3) * 4);

    #[cfg(target_arch = "aarch64")]
    rgb_to_rgba_neon(src, dst);

    #[cfg(target_arch = "x86_64")]
    // SAFETY: runtime feature detection gates the SSSE3 path
    unsafe {
        rgb_to_rgba_x86(src, dst);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    rgb_to_rgba_scalar(src, dst);
}

/// Expand BGR888 to RGBA8888 (swap R/B + alpha=255) using SIMD where available.
#[inline]
pub(crate) fn bgr_to_rgba_simd(src: &[u8], dst: &mut [u8]) {
    assert!(src.len().is_multiple_of(3));
    assert_eq!(dst.len(), (src.len() / 3) * 4);

    #[cfg(target_arch = "aarch64")]
    bgr_to_rgba_neon(src, dst);

    #[cfg(target_arch = "x86_64")]
    // SAFETY: runtime feature detection gates the SSSE3 path
    unsafe {
        bgr_to_rgba_x86(src, dst);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    bgr_to_rgba_scalar(src, dst);
}

#[allow(dead_code)]
fn rgb_to_rgba_scalar(src: &[u8], dst: &mut [u8]) {
    for (rgb, rgba) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
        rgba[0] = rgb[0];
        rgba[1] = rgb[1];
        rgba[2] = rgb[2];
        rgba[3] = 255;
    }
}

#[allow(dead_code)]
fn bgr_to_rgba_scalar(src: &[u8], dst: &mut [u8]) {
    for (bgr, rgba) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
        rgba[0] = bgr[2];
        rgba[1] = bgr[1];
        rgba[2] = bgr[0];
        rgba[3] = 255;
    }
}

#[cfg(target_arch = "aarch64")]
fn rgb_to_rgba_neon(src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::{uint8x16x3_t, uint8x16x4_t, vdupq_n_u8, vld3q_u8, vst4q_u8};

    // Process 16 RGB pixels (48 bytes) → 16 RGBA pixels (64 bytes) per iteration
    let simd_end = src.len() - (src.len() % 48);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        // SAFETY: si + 48 <= src.len() and di + 64 <= dst.len()
        unsafe {
            let rgb: uint8x16x3_t = vld3q_u8(src.as_ptr().add(si));
            let alpha = vdupq_n_u8(255);
            vst4q_u8(
                dst.as_mut_ptr().add(di),
                uint8x16x4_t(rgb.0, rgb.1, rgb.2, alpha),
            );
        }
        si += 48;
        di += 64;
    }

    rgb_to_rgba_scalar(&src[si..], &mut dst[di..]);
}

#[cfg(target_arch = "aarch64")]
fn bgr_to_rgba_neon(src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::{uint8x16x3_t, uint8x16x4_t, vdupq_n_u8, vld3q_u8, vst4q_u8};

    let simd_end = src.len() - (src.len() % 48);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        // SAFETY: si + 48 <= src.len() and di + 64 <= dst.len()
        unsafe {
            let bgr: uint8x16x3_t = vld3q_u8(src.as_ptr().add(si));
            let alpha = vdupq_n_u8(255);
            // Swap B and R channels
            vst4q_u8(
                dst.as_mut_ptr().add(di),
                uint8x16x4_t(bgr.2, bgr.1, bgr.0, alpha),
            );
        }
        si += 48;
        di += 64;
    }

    bgr_to_rgba_scalar(&src[si..], &mut dst[di..]);
}

#[cfg(target_arch = "x86_64")]
unsafe fn rgb_to_rgba_x86(src: &[u8], dst: &mut [u8]) {
    if is_x86_feature_detected!("ssse3") {
        rgb_to_rgba_ssse3(src, dst);
    } else {
        rgb_to_rgba_scalar(src, dst);
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn bgr_to_rgba_x86(src: &[u8], dst: &mut [u8]) {
    if is_x86_feature_detected!("ssse3") {
        bgr_to_rgba_ssse3(src, dst);
    } else {
        bgr_to_rgba_scalar(src, dst);
    }
}

/// Expand RGB→RGBA using SSSE3 pshufb: process 4 pixels (12 bytes → 16 bytes) per iteration.
/// Loads 16 bytes, shuffles 12 into RGBA layout with alpha=0xFF via OR.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn rgb_to_rgba_ssse3(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::{
        _mm_loadu_si128, _mm_or_si128, _mm_set1_epi32, _mm_setr_epi8, _mm_shuffle_epi8,
        _mm_storeu_si128,
    };

    // Shuffle: take 12 bytes of RGB and arrange into RGBA with byte 3,7,11,15 = 0 (masked)
    // Input bytes: R0 G0 B0 R1 G1 B1 R2 G2 B2 R3 G3 B3 ...
    // Output:      R0 G0 B0 FF R1 G1 B1 FF R2 G2 B2 FF R3 G3 B3 FF
    let shuf = _mm_setr_epi8(0, 1, 2, -1, 3, 4, 5, -1, 6, 7, 8, -1, 9, 10, 11, -1);
    // Alpha mask: 0xFF in positions 3,7,11,15
    let alpha = _mm_set1_epi32(i32::from_ne_bytes([0, 0, 0, 0xFF]));

    let simd_limit = src.len().saturating_sub(15); // need 16 readable bytes
    let mut si = 0;
    let mut di = 0;

    while si < simd_limit {
        // SAFETY: si + 16 <= src.len()
        let rgb = _mm_loadu_si128(src.as_ptr().add(si).cast());
        let expanded = _mm_shuffle_epi8(rgb, shuf);
        let with_alpha = _mm_or_si128(expanded, alpha);
        _mm_storeu_si128(dst.as_mut_ptr().add(di).cast(), with_alpha);
        si += 12; // consumed 4 RGB pixels
        di += 16; // produced 4 RGBA pixels
    }

    rgb_to_rgba_scalar(&src[si..], &mut dst[di..]);
}

/// Expand BGR→RGBA using SSSE3 pshufb (swap R/B + insert alpha).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn bgr_to_rgba_ssse3(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::{
        _mm_loadu_si128, _mm_or_si128, _mm_set1_epi32, _mm_setr_epi8, _mm_shuffle_epi8,
        _mm_storeu_si128,
    };

    // BGR→RGBA: swap B/R in each 3-byte group and insert alpha
    // Input:  B0 G0 R0 B1 G1 R1 B2 G2 R2 B3 G3 R3 ...
    // Output: R0 G0 B0 FF R1 G1 B1 FF R2 G2 B2 FF R3 G3 B3 FF
    let shuf = _mm_setr_epi8(2, 1, 0, -1, 5, 4, 3, -1, 8, 7, 6, -1, 11, 10, 9, -1);
    let alpha = _mm_set1_epi32(i32::from_ne_bytes([0, 0, 0, 0xFF]));

    let simd_limit = src.len().saturating_sub(15);
    let mut si = 0;
    let mut di = 0;

    while si < simd_limit {
        let bgr = _mm_loadu_si128(src.as_ptr().add(si).cast());
        let expanded = _mm_shuffle_epi8(bgr, shuf);
        let with_alpha = _mm_or_si128(expanded, alpha);
        _mm_storeu_si128(dst.as_mut_ptr().add(di).cast(), with_alpha);
        si += 12;
        di += 16;
    }

    bgr_to_rgba_scalar(&src[si..], &mut dst[di..]);
}

// ──────────────────────────────────────────────
// YUYV Y-channel extraction  (stride-2 byte pick)
// ──────────────────────────────────────────────

/// Extract every other byte from YUYV data (Y0, Y1, Y2, ...) using SIMD.
/// `src.len()` must be a multiple of 4, `dst.len()` must be `src.len() / 2`.
#[inline]
pub(crate) fn yuyv_extract_luma_simd(src: &[u8], dst: &mut [u8]) {
    assert!(src.len().is_multiple_of(4));
    assert_eq!(dst.len(), src.len() / 2);

    #[cfg(target_arch = "aarch64")]
    yuyv_extract_luma_neon(src, dst);

    #[cfg(target_arch = "x86_64")]
    // SAFETY: runtime feature detection gates the SSSE3 path
    unsafe {
        yuyv_extract_luma_x86(src, dst);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    yuyv_extract_luma_scalar(src, dst);
}

#[allow(dead_code)]
fn yuyv_extract_luma_scalar(src: &[u8], dst: &mut [u8]) {
    for (chunk, out) in src.chunks_exact(4).zip(dst.chunks_exact_mut(2)) {
        out[0] = chunk[0]; // Y0
        out[1] = chunk[2]; // Y1
    }
}

#[cfg(target_arch = "aarch64")]
fn yuyv_extract_luma_neon(src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::{vld2q_u8, vst1q_u8};

    // vld2q_u8 loads 32 bytes and deinterleaves into even/odd:
    // .0 = bytes 0,2,4,...,30 (Y values), .1 = bytes 1,3,5,...,31 (U/V values)
    let simd_end = src.len() - (src.len() % 32);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        // SAFETY: si + 32 <= src.len() and di + 16 <= dst.len()
        unsafe {
            let deinterleaved = vld2q_u8(src.as_ptr().add(si));
            vst1q_u8(dst.as_mut_ptr().add(di), deinterleaved.0); // Y bytes only
        }
        si += 32;
        di += 16;
    }

    yuyv_extract_luma_scalar(&src[si..], &mut dst[di..]);
}

#[cfg(target_arch = "x86_64")]
unsafe fn yuyv_extract_luma_x86(src: &[u8], dst: &mut [u8]) {
    if is_x86_feature_detected!("ssse3") {
        yuyv_extract_luma_ssse3(src, dst);
    } else {
        yuyv_extract_luma_scalar(src, dst);
    }
}

/// Extract Y bytes from YUYV using SSSE3 pshufb.
/// Loads 16 bytes (4 YUYV pairs = 8 pixels), shuffles out 8 Y bytes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn yuyv_extract_luma_ssse3(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::{_mm_loadu_si128, _mm_setr_epi8, _mm_shuffle_epi8, _mm_storel_epi64};

    // Select even bytes: positions 0,2,4,6,8,10,12,14 → low 8 bytes of result
    let shuf = _mm_setr_epi8(0, 2, 4, 6, 8, 10, 12, 14, -1, -1, -1, -1, -1, -1, -1, -1);

    let simd_end = src.len() - (src.len() % 16);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        // SAFETY: si + 16 <= src.len() and di + 8 <= dst.len()
        let yuyv = _mm_loadu_si128(src.as_ptr().add(si).cast());
        let y_bytes = _mm_shuffle_epi8(yuyv, shuf);
        _mm_storel_epi64(dst.as_mut_ptr().add(di).cast(), y_bytes);
        si += 16;
        di += 8;
    }

    yuyv_extract_luma_scalar(&src[si..], &mut dst[di..]);
}

// ──────────────────────────────────────────────
// RGB → Luma averaging  ((R+G+B)/3)
// ──────────────────────────────────────────────

/// Compute `(R+G+B)/3` per pixel using SIMD where available.
/// `src.len()` must be a multiple of 3, `dst.len()` must be `src.len() / 3`.
#[inline]
pub(crate) fn rgb_to_luma_simd(src: &[u8], dst: &mut [u8]) {
    assert!(src.len().is_multiple_of(3));
    assert_eq!(dst.len(), src.len() / 3);

    #[cfg(target_arch = "aarch64")]
    rgb_to_luma_neon(src, dst);

    #[cfg(target_arch = "x86_64")]
    // SAFETY: runtime feature detection gates the SSE2 path
    unsafe {
        rgb_to_luma_x86(src, dst);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    rgb_to_luma_scalar(src, dst);
}

#[allow(dead_code, clippy::cast_possible_truncation)]
fn rgb_to_luma_scalar(src: &[u8], dst: &mut [u8]) {
    for (rgb, out) in src.chunks_exact(3).zip(dst.iter_mut()) {
        *out = ((u16::from(rgb[0]) + u16::from(rgb[1]) + u16::from(rgb[2])) / 3) as u8;
    }
}

#[cfg(target_arch = "aarch64")]
fn rgb_to_luma_neon(src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::{
        vaddl_u8, vaddw_u8, vcombine_u16, vcombine_u8, vdupq_n_u16, vget_high_u16, vget_high_u8,
        vget_low_u16, vget_low_u8, vld3q_u8, vmull_u16, vqmovn_u16, vshrn_n_u32, vst1q_u8,
    };

    // Process 16 RGB pixels (48 bytes) → 16 luma bytes per iteration
    let simd_end = src.len() - (src.len() % 48);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        // SAFETY: si + 48 <= src.len() and di + 16 <= dst.len()
        unsafe {
            let k21846 = vdupq_n_u16(21846);
            let rgb = vld3q_u8(src.as_ptr().add(si));

            // Low 8 pixels: widen to u16, sum, then (sum * 21846) >> 16 ≈ sum/3
            let sum_lo = vaddw_u8(
                vaddl_u8(vget_low_u8(rgb.0), vget_low_u8(rgb.1)),
                vget_low_u8(rgb.2),
            );
            let prod_lo_lo = vmull_u16(vget_low_u16(sum_lo), vget_low_u16(k21846));
            let prod_lo_hi = vmull_u16(vget_high_u16(sum_lo), vget_high_u16(k21846));
            let res_lo = vcombine_u16(vshrn_n_u32::<16>(prod_lo_lo), vshrn_n_u32::<16>(prod_lo_hi));
            let luma_lo = vqmovn_u16(res_lo);

            // High 8 pixels
            let sum_hi = vaddw_u8(
                vaddl_u8(vget_high_u8(rgb.0), vget_high_u8(rgb.1)),
                vget_high_u8(rgb.2),
            );
            let prod_hi_lo = vmull_u16(vget_low_u16(sum_hi), vget_low_u16(k21846));
            let prod_hi_hi = vmull_u16(vget_high_u16(sum_hi), vget_high_u16(k21846));
            let res_hi = vcombine_u16(vshrn_n_u32::<16>(prod_hi_lo), vshrn_n_u32::<16>(prod_hi_hi));
            let luma_hi = vqmovn_u16(res_hi);

            vst1q_u8(dst.as_mut_ptr().add(di), vcombine_u8(luma_lo, luma_hi));
        }
        si += 48;
        di += 16;
    }

    rgb_to_luma_scalar(&src[si..], &mut dst[di..]);
}

#[cfg(target_arch = "x86_64")]
unsafe fn rgb_to_luma_x86(src: &[u8], dst: &mut [u8]) {
    // SSE2 is always available on x86_64
    rgb_to_luma_sse2(src, dst);
}

/// Compute (R+G+B)/3 per pixel using SSE2.
/// Processes 4 pixels (12 bytes → 4 luma bytes) per iteration.
#[cfg(target_arch = "x86_64")]
#[allow(clippy::cast_possible_truncation)]
unsafe fn rgb_to_luma_sse2(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::{
        _mm_add_epi16, _mm_mulhi_epu16, _mm_packus_epi16, _mm_set1_epi16, _mm_setzero_si128,
        _mm_storeu_si128, _mm_unpacklo_epi8,
    };

    let zero = _mm_setzero_si128();
    let k21846 = _mm_set1_epi16(21846_i16); // 65536/3 ≈ 21845.33, round to 21846

    // Process 8 pixels (24 bytes) per iteration using two loads
    let simd_end = src.len() - (src.len() % 24);
    let mut si = 0;
    let mut di = 0;

    while si < simd_end {
        // Load 8 RGB pixels (24 bytes) and manually deinterleave
        let mut r_arr = [0u8; 8];
        let mut g_arr = [0u8; 8];
        let mut b_arr = [0u8; 8];
        for px in 0..8 {
            r_arr[px] = *src.get_unchecked(si + px * 3);
            g_arr[px] = *src.get_unchecked(si + px * 3 + 1);
            b_arr[px] = *src.get_unchecked(si + px * 3 + 2);
        }

        // Load into SSE registers as u8, widen to u16
        let r8 = _mm_unpacklo_epi8(
            std::arch::x86_64::_mm_loadl_epi64(r_arr.as_ptr().cast()),
            zero,
        );
        let g8 = _mm_unpacklo_epi8(
            std::arch::x86_64::_mm_loadl_epi64(g_arr.as_ptr().cast()),
            zero,
        );
        let b8 = _mm_unpacklo_epi8(
            std::arch::x86_64::_mm_loadl_epi64(b_arr.as_ptr().cast()),
            zero,
        );

        // Sum: 8x u16
        let sum = _mm_add_epi16(_mm_add_epi16(r8, g8), b8);

        // Divide by 3: (sum * 21846) >> 16, using _mm_mulhi_epu16 which gives high 16 bits
        let result_16 = _mm_mulhi_epu16(sum, k21846);

        // Pack u16 → u8 (saturating)
        let result_8 = _mm_packus_epi16(result_16, zero);

        // Store 8 bytes
        let mut tmp = [0u8; 16];
        _mm_storeu_si128(tmp.as_mut_ptr().cast(), result_8);
        std::ptr::copy_nonoverlapping(tmp.as_ptr(), dst.as_mut_ptr().add(di), 8);

        si += 24;
        di += 8;
    }

    // Scalar tail
    rgb_to_luma_scalar(&src[si..], &mut dst[di..]);
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
    fn bgr_to_rgb_non_aligned_exercises_tail() {
        // 103 pixels = 309 bytes, not a multiple of 15 (SSSE3) or 24 (NEON),
        // so the tail path is exercised on both architectures.
        let pixel_count = 103;
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

    // ─── NV12 tests ───

    #[test]
    fn nv12_to_rgb_matches_scalar() {
        // 4×2 image: smallest valid NV12 (width and height even)
        let width = 4;
        let height = 2;
        let y_plane: Vec<u8> = vec![128, 64, 200, 16, 235, 100, 50, 180];
        let uv_plane: Vec<u8> = vec![128, 128, 200, 50, 128, 128, 200, 50]; // shared for both rows
        let data: Vec<u8> = [y_plane, uv_plane].concat();

        let mut simd_rgb = vec![0u8; width * height * 3];
        let mut scalar_rgb = vec![0u8; width * height * 3];

        nv12_to_rgb_simd(width, height, &data, &mut simd_rgb, false);
        nv12_to_rgb_scalar(width, height, &data, &mut scalar_rgb, false);

        assert_eq!(simd_rgb, scalar_rgb, "NV12→RGB: SIMD and scalar must match");
    }

    #[test]
    fn nv12_to_rgba_matches_scalar() {
        let width = 4;
        let height = 2;
        let y_plane: Vec<u8> = vec![128, 64, 200, 16, 235, 100, 50, 180];
        let uv_plane: Vec<u8> = vec![128, 128, 200, 50, 128, 128, 200, 50];
        let data: Vec<u8> = [y_plane, uv_plane].concat();

        let mut simd_rgba = vec![0u8; width * height * 4];
        let mut scalar_rgba = vec![0u8; width * height * 4];

        nv12_to_rgb_simd(width, height, &data, &mut simd_rgba, true);
        nv12_to_rgb_scalar(width, height, &data, &mut scalar_rgba, true);

        assert_eq!(
            simd_rgba, scalar_rgba,
            "NV12→RGBA: SIMD and scalar must match"
        );
    }

    #[test]
    fn nv12_to_rgb_large_matches_scalar() {
        // 32×4: large enough to exercise SIMD paths (≥16 pixels/row for NEON, ≥8 for SSE)
        let width = 32;
        let height = 4;
        let y_size = width * height;
        let uv_size = width * (height / 2);
        let data: Vec<u8> = (0..y_size + uv_size).map(|i| (i * 7 % 256) as u8).collect();

        let mut simd_rgb = vec![0u8; width * height * 3];
        let mut scalar_rgb = vec![0u8; width * height * 3];

        nv12_to_rgb_simd(width, height, &data, &mut simd_rgb, false);
        nv12_to_rgb_scalar(width, height, &data, &mut scalar_rgb, false);

        assert_eq!(
            simd_rgb, scalar_rgb,
            "NV12→RGB large: SIMD and scalar must match"
        );
    }

    #[test]
    fn nv12_to_rgba_large_matches_scalar() {
        let width = 32;
        let height = 4;
        let y_size = width * height;
        let uv_size = width * (height / 2);
        let data: Vec<u8> = (0..y_size + uv_size).map(|i| (i * 7 % 256) as u8).collect();

        let mut simd_rgba = vec![0u8; width * height * 4];
        let mut scalar_rgba = vec![0u8; width * height * 4];

        nv12_to_rgb_simd(width, height, &data, &mut simd_rgba, true);
        nv12_to_rgb_scalar(width, height, &data, &mut scalar_rgba, true);

        assert_eq!(
            simd_rgba, scalar_rgba,
            "NV12→RGBA large: SIMD and scalar must match"
        );
    }

    #[test]
    fn nv12_to_rgb_non_aligned_tail() {
        // 18×2: 18 is not a multiple of 16 (NEON) or 8 (SSE), exercises tail path
        let width = 18;
        let height = 2;
        let y_size = width * height;
        let uv_size = width * (height / 2);
        let data: Vec<u8> = (0..y_size + uv_size)
            .map(|i| (i * 13 % 256) as u8)
            .collect();

        let mut simd_rgb = vec![0u8; width * height * 3];
        let mut scalar_rgb = vec![0u8; width * height * 3];

        nv12_to_rgb_simd(width, height, &data, &mut simd_rgb, false);
        nv12_to_rgb_scalar(width, height, &data, &mut scalar_rgb, false);

        assert_eq!(
            simd_rgb, scalar_rgb,
            "NV12→RGB non-aligned: SIMD and scalar must match"
        );
    }

    #[test]
    fn nv12_to_rgba_non_aligned_tail() {
        let width = 18;
        let height = 2;
        let y_size = width * height;
        let uv_size = width * (height / 2);
        let data: Vec<u8> = (0..y_size + uv_size)
            .map(|i| (i * 13 % 256) as u8)
            .collect();

        let mut simd_rgba = vec![0u8; width * height * 4];
        let mut scalar_rgba = vec![0u8; width * height * 4];

        nv12_to_rgb_simd(width, height, &data, &mut simd_rgba, true);
        nv12_to_rgb_scalar(width, height, &data, &mut scalar_rgba, true);

        assert_eq!(
            simd_rgba, scalar_rgba,
            "NV12→RGBA non-aligned: SIMD and scalar must match"
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

    // ─── RGB/BGR → RGBA tests ───

    #[test]
    fn rgb_to_rgba_matches_scalar() {
        let rgb: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect(); // 100 pixels
        let mut simd_out = vec![0u8; (rgb.len() / 3) * 4];
        let mut scalar_out = vec![0u8; (rgb.len() / 3) * 4];

        rgb_to_rgba_simd(&rgb, &mut simd_out);
        rgb_to_rgba_scalar(&rgb, &mut scalar_out);

        assert_eq!(simd_out, scalar_out, "RGB→RGBA: SIMD and scalar must match");
    }

    #[test]
    fn bgr_to_rgba_matches_scalar() {
        let bgr: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
        let mut simd_out = vec![0u8; (bgr.len() / 3) * 4];
        let mut scalar_out = vec![0u8; (bgr.len() / 3) * 4];

        bgr_to_rgba_simd(&bgr, &mut simd_out);
        bgr_to_rgba_scalar(&bgr, &mut scalar_out);

        assert_eq!(simd_out, scalar_out, "BGR→RGBA: SIMD and scalar must match");
    }

    #[test]
    fn rgb_to_rgba_non_aligned_tail() {
        // 7 pixels = 21 bytes, not a multiple of 48 (NEON) or 12 (SSE)
        let rgb: Vec<u8> = (0..21).map(|i| (i * 11 % 256) as u8).collect();
        let mut simd_out = vec![0u8; 28];
        let mut scalar_out = vec![0u8; 28];

        rgb_to_rgba_simd(&rgb, &mut simd_out);
        rgb_to_rgba_scalar(&rgb, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "RGB→RGBA non-aligned: SIMD and scalar must match"
        );
    }

    #[test]
    fn bgr_to_rgba_non_aligned_tail() {
        let bgr: Vec<u8> = (0..21).map(|i| (i * 11 % 256) as u8).collect();
        let mut simd_out = vec![0u8; 28];
        let mut scalar_out = vec![0u8; 28];

        bgr_to_rgba_simd(&bgr, &mut simd_out);
        bgr_to_rgba_scalar(&bgr, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "BGR→RGBA non-aligned: SIMD and scalar must match"
        );
    }

    #[test]
    fn rgb_to_rgba_alpha_is_255() {
        let rgb = vec![10u8, 20, 30, 40, 50, 60];
        let mut rgba = vec![0u8; 8];
        rgb_to_rgba_simd(&rgb, &mut rgba);
        assert_eq!(rgba[3], 255, "Alpha must be 255");
        assert_eq!(rgba[7], 255, "Alpha must be 255");
    }

    // ─── YUYV Y-channel extraction tests ───

    #[test]
    fn yuyv_extract_luma_matches_scalar() {
        // 16 YUYV pairs = 64 bytes → 32 Y pixels
        let yuyv: Vec<u8> = (0..64).map(|i| (i * 7 % 256) as u8).collect();
        let mut simd_out = vec![0u8; 32];
        let mut scalar_out = vec![0u8; 32];

        yuyv_extract_luma_simd(&yuyv, &mut simd_out);
        yuyv_extract_luma_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "YUYV luma extraction: SIMD and scalar must match"
        );
    }

    #[test]
    fn yuyv_extract_luma_large() {
        // 100 YUYV pairs = 400 bytes → 200 Y pixels (exercises SIMD main loop)
        let yuyv: Vec<u8> = (0..400).map(|i| (i * 13 % 256) as u8).collect();
        let mut simd_out = vec![0u8; 200];
        let mut scalar_out = vec![0u8; 200];

        yuyv_extract_luma_simd(&yuyv, &mut simd_out);
        yuyv_extract_luma_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "YUYV luma extraction large: SIMD and scalar must match"
        );
    }

    #[test]
    fn yuyv_extract_luma_non_aligned() {
        // 5 YUYV pairs = 20 bytes, not multiple of 16 (SSE) or 32 (NEON)
        let yuyv: Vec<u8> = (0..20).map(|i| (i * 11 % 256) as u8).collect();
        let mut simd_out = vec![0u8; 10];
        let mut scalar_out = vec![0u8; 10];

        yuyv_extract_luma_simd(&yuyv, &mut simd_out);
        yuyv_extract_luma_scalar(&yuyv, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "YUYV luma extraction non-aligned: SIMD and scalar must match"
        );
    }

    #[test]
    fn yuyv_extract_luma_known_values() {
        // Verify correct byte selection
        let yuyv = vec![10u8, 20, 30, 40, 50, 60, 70, 80];
        let mut out = vec![0u8; 4];
        yuyv_extract_luma_simd(&yuyv, &mut out);
        assert_eq!(
            out,
            vec![10, 30, 50, 70],
            "Should extract Y0,Y1 from each pair"
        );
    }

    // ─── RGB → Luma averaging tests ───

    #[test]
    fn rgb_to_luma_matches_scalar() {
        let rgb: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect(); // 100 pixels
        let mut simd_out = vec![0u8; 100];
        let mut scalar_out = vec![0u8; 100];

        rgb_to_luma_simd(&rgb, &mut simd_out);
        rgb_to_luma_scalar(&rgb, &mut scalar_out);

        assert_eq!(simd_out, scalar_out, "RGB→Luma: SIMD and scalar must match");
    }

    #[test]
    fn rgb_to_luma_large() {
        // 200 pixels = 600 bytes, exercises SIMD main loop
        let rgb: Vec<u8> = (0..600).map(|i| (i * 7 % 256) as u8).collect();
        let mut simd_out = vec![0u8; 200];
        let mut scalar_out = vec![0u8; 200];

        rgb_to_luma_simd(&rgb, &mut simd_out);
        rgb_to_luma_scalar(&rgb, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "RGB→Luma large: SIMD and scalar must match"
        );
    }

    #[test]
    fn rgb_to_luma_non_aligned() {
        // 5 pixels = 15 bytes, not multiple of 48 (NEON) or 24 (SSE)
        let rgb: Vec<u8> = (0..15).map(|i| (i * 13 % 256) as u8).collect();
        let mut simd_out = vec![0u8; 5];
        let mut scalar_out = vec![0u8; 5];

        rgb_to_luma_simd(&rgb, &mut simd_out);
        rgb_to_luma_scalar(&rgb, &mut scalar_out);

        assert_eq!(
            simd_out, scalar_out,
            "RGB→Luma non-aligned: SIMD and scalar must match"
        );
    }

    #[test]
    fn rgb_to_luma_known_values() {
        // (255+0+0)/3=85, (0+255+0)/3=85, (0+0+255)/3=85, (255+255+255)/3=255
        let rgb = vec![255u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let mut out = vec![0u8; 4];
        rgb_to_luma_simd(&rgb, &mut out);
        assert_eq!(out, vec![85, 85, 85, 255]);
    }
}
