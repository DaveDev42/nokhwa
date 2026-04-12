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

//! YUYV → RGB / RGBA (YUV 4:2:2) — NEON / SSE4.1 / scalar.

#[cfg(test)]
use crate::types::yuyv444_to_rgb;
#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::{int16x8_t, int32x4_t, uint8x8_t};

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
pub(super) unsafe fn yuv_to_rgb_neon_8px(
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
        _mm_add_epi32, _mm_cvtepi16_epi32, _mm_loadu_si128, _mm_max_epi16, _mm_mullo_epi32,
        _mm_packs_epi32, _mm_packus_epi16, _mm_set1_epi16, _mm_set1_epi32, _mm_setr_epi8,
        _mm_setzero_si128, _mm_shuffle_epi8, _mm_srai_epi32, _mm_srli_si128, _mm_storeu_si128,
        _mm_sub_epi16, _mm_sub_epi32, _mm_unpacklo_epi8,
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

#[cfg(test)]
mod tests {
    use super::*;

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
