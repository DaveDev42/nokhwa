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

//! BGR → RGB (3-byte channel swap) — NEON / SSSE3+AVX2 / scalar.

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
        bgr_to_rgb_x86(src, dst);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    bgr_to_rgb_scalar(src, dst);
}

/// Scalar BGR-to-RGB fallback.
#[inline]
pub(crate) fn bgr_to_rgb_scalar(src: &[u8], dst: &mut [u8]) {
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
unsafe fn bgr_to_rgb_x86(src: &[u8], dst: &mut [u8]) {
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
    fn bgr_to_rgb_empty() {
        let bgr: Vec<u8> = vec![];
        let mut rgb: Vec<u8> = vec![];
        bgr_to_rgb_simd(&bgr, &mut rgb);
        assert!(rgb.is_empty());
    }
}
