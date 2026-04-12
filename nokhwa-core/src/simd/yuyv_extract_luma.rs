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

//! YUYV Y-channel extraction (stride-2 byte pick) — NEON / SSSE3 / scalar.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
