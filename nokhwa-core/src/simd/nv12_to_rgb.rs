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

//! NV12 → RGB / RGBA (YUV 4:2:0 bi-planar) — NEON / SSE4.1 / scalar.

#[cfg(target_arch = "aarch64")]
use super::yuyv_to_rgb::yuv_to_rgb_neon_8px;

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
pub fn nv12_to_rgb_simd(width: usize, height: usize, data: &[u8], out: &mut [u8], rgba: bool) {
    let pxsize = if rgba { 4 } else { 3 };
    assert!(width.is_multiple_of(2) && height.is_multiple_of(2));
    assert_eq!(data.len(), width * height * 3 / 2);
    assert_eq!(out.len(), width * height * pxsize);

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
pub fn nv12_to_rgb_scalar(width: usize, height: usize, data: &[u8], out: &mut [u8], rgba: bool) {
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

/// Process NV12 using SSE4.1 (available on all `x86_64` CPUs since ~2008).
/// Processes 8 Y pixels + 4 UV pairs per iteration using `_mm_mullo_epi32` (SSE4.1).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
#[allow(clippy::similar_names, clippy::cast_sign_loss, clippy::too_many_lines)]
unsafe fn nv12_to_rgb_sse41(width: usize, height: usize, data: &[u8], out: &mut [u8], rgba: bool) {
    use std::arch::x86_64::{
        _mm_add_epi32, _mm_cvtepi16_epi32, _mm_loadl_epi64, _mm_max_epi16, _mm_mullo_epi32,
        _mm_packs_epi32, _mm_packus_epi16, _mm_set1_epi16, _mm_set1_epi32, _mm_setr_epi8,
        _mm_setzero_si128, _mm_shuffle_epi8, _mm_srai_epi32, _mm_srli_si128, _mm_storeu_si128,
        _mm_sub_epi16, _mm_sub_epi32, _mm_unpacklo_epi8,
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

#[cfg(test)]
mod tests {
    use super::*;

    // ─── NV12 tests ───

    #[test]
    fn nv12_to_rgb_matches_scalar() {
        // 4×2 image: smallest valid NV12 (width and height even)
        // UV plane has 1 row (height/2) with 2 UV pairs (width/2) = 4 bytes
        let width = 4;
        let height = 2;
        let y_plane: Vec<u8> = vec![128, 64, 200, 16, 235, 100, 50, 180];
        let uv_plane: Vec<u8> = vec![128, 128, 200, 50];
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
        let uv_plane: Vec<u8> = vec![128, 128, 200, 50];
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
}
