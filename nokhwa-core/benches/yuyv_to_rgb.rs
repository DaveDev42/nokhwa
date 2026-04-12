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
    yuyv_to_rgb_scalar, yuyv_to_rgb_simd, yuyv_to_rgba_scalar, yuyv_to_rgba_simd,
};

mod common;
use common::{pattern, SIZES};

fn verify() {
    let src = pattern(32);
    let mut a = vec![0u8; 48];
    let mut b = vec![0u8; 48];
    yuyv_to_rgb_simd(&src, &mut a);
    yuyv_to_rgb_scalar(&src, &mut b);
    assert_eq!(a, b, "yuyv_to_rgb SIMD vs scalar mismatch");

    let mut a = vec![0u8; 64];
    let mut b = vec![0u8; 64];
    yuyv_to_rgba_simd(&src, &mut a);
    yuyv_to_rgba_scalar(&src, &mut b);
    assert_eq!(a, b, "yuyv_to_rgba SIMD vs scalar mismatch");
}

fn bench(c: &mut Criterion) {
    verify();

    {
        let mut group = c.benchmark_group("yuyv_to_rgb");
        for &(w, h) in SIZES {
            let pixels = w * h;
            let src_len = pixels * 2;
            let dst_len = pixels * 3;
            let src = pattern(src_len);
            let mut dst = vec![0u8; dst_len];
            group.throughput(Throughput::Bytes(src_len as u64));
            let id = format!("{w}x{h}");
            group.bench_with_input(BenchmarkId::new("simd", &id), &src, |b, src| {
                b.iter(|| yuyv_to_rgb_simd(black_box(src), black_box(&mut dst)));
            });
            group.bench_with_input(BenchmarkId::new("scalar", &id), &src, |b, src| {
                b.iter(|| yuyv_to_rgb_scalar(black_box(src), black_box(&mut dst)));
            });
        }
        group.finish();
    }

    let mut group = c.benchmark_group("yuyv_to_rgba");
    for &(w, h) in SIZES {
        let pixels = w * h;
        let src_len = pixels * 2;
        let dst_len = pixels * 4;
        let src = pattern(src_len);
        let mut dst = vec![0u8; dst_len];
        group.throughput(Throughput::Bytes(src_len as u64));
        let id = format!("{w}x{h}");
        group.bench_with_input(BenchmarkId::new("simd", &id), &src, |b, src| {
            b.iter(|| yuyv_to_rgba_simd(black_box(src), black_box(&mut dst)));
        });
        group.bench_with_input(BenchmarkId::new("scalar", &id), &src, |b, src| {
            b.iter(|| yuyv_to_rgba_scalar(black_box(src), black_box(&mut dst)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
