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

//! RGB / BGR → RGBA (3-byte to 4-byte expansion) — NEON / SSSE3 / scalar.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
