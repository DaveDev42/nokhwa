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

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nokhwa_core::bench_exports::{
    nv12_to_rgb_scalar, nv12_to_rgb_simd, nv12_to_rgba_scalar, nv12_to_rgba_simd,
};

mod common;
use common::{pattern, SIZES};

fn verify() {
    // Non-multiple of the 16-pixel SIMD block so the scalar tail is exercised.
    let (w, h) = (20usize, 18usize);
    let src = pattern(w * h * 3 / 2);
    let mut a = vec![0u8; w * h * 3];
    let mut b = vec![0u8; w * h * 3];
    nv12_to_rgb_simd(w, h, &src, &mut a);
    nv12_to_rgb_scalar(w, h, &src, &mut b);
    assert_eq!(a, b, "nv12_to_rgb SIMD vs scalar mismatch");

    let mut a = vec![0u8; w * h * 4];
    let mut b = vec![0u8; w * h * 4];
    nv12_to_rgba_simd(w, h, &src, &mut a);
    nv12_to_rgba_scalar(w, h, &src, &mut b);
    assert_eq!(a, b, "nv12_to_rgba SIMD vs scalar mismatch");
}

fn bench(c: &mut Criterion) {
    verify();

    for &rgba in &[false, true] {
        let pxsize = if rgba { 4 } else { 3 };
        let label = if rgba { "nv12_to_rgba" } else { "nv12_to_rgb" };
        let mut group = c.benchmark_group(label);
        for &(w, h) in SIZES {
            let src_len = w * h * 3 / 2;
            let dst_len = w * h * pxsize;
            let src = pattern(src_len);
            let mut dst = vec![0u8; dst_len];
            group.throughput(Throughput::Bytes(u64::try_from(src_len).unwrap()));
            let id = format!("{w}x{h}");
            group.bench_with_input(BenchmarkId::new("simd", &id), &src, |b, src| {
                b.iter(|| {
                    if rgba {
                        nv12_to_rgba_simd(w, h, black_box(src), black_box(&mut dst));
                    } else {
                        nv12_to_rgb_simd(w, h, black_box(src), black_box(&mut dst));
                    }
                });
            });
            group.bench_with_input(BenchmarkId::new("scalar", &id), &src, |b, src| {
                b.iter(|| {
                    if rgba {
                        nv12_to_rgba_scalar(w, h, black_box(src), black_box(&mut dst));
                    } else {
                        nv12_to_rgb_scalar(w, h, black_box(src), black_box(&mut dst));
                    }
                });
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
