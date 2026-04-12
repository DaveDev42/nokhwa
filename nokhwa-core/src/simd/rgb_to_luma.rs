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

//! RGB → Luma averaging ((R+G+B)/3) — NEON / SSE2 / scalar.

// ──────────────────────────────────────────────
// RGB → Luma averaging  ((R+G+B)/3)
// ──────────────────────────────────────────────

/// Compute `(R+G+B)/3` per pixel using SIMD where available.
/// `src.len()` must be a multiple of 3, `dst.len()` must be `src.len() / 3`.
#[inline]
pub fn rgb_to_luma_simd(src: &[u8], dst: &mut [u8]) {
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
pub fn rgb_to_luma_scalar(src: &[u8], dst: &mut [u8]) {
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
        _mm_unpacklo_epi8,
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

        // Store 8 luma bytes directly
        std::arch::x86_64::_mm_storel_epi64(dst.as_mut_ptr().add(di).cast(), result_8);

        si += 24;
        di += 8;
    }

    // Scalar tail
    rgb_to_luma_scalar(&src[si..], &mut dst[di..]);
}

#[cfg(test)]
mod tests {
    use super::*;

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
