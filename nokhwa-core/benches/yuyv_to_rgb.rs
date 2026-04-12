use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use nokhwa_core::simd::bench_exports::{
    yuyv_to_rgb_scalar, yuyv_to_rgb_simd, yuyv_to_rgba_scalar, yuyv_to_rgba_simd,
};

const SIZES: &[(u32, u32)] = &[(640, 480), (1920, 1080), (3840, 2160)];

fn bench(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("yuyv_to_rgb");
        for &(w, h) in SIZES {
            let pixels = (w as usize) * (h as usize);
            let src_len = pixels * 2;
            let dst_len = pixels * 3;
            let src = vec![128u8; src_len];
            let mut dst = vec![0u8; dst_len];
            group.throughput(Throughput::Bytes(src_len as u64));
            let id = format!("{}x{}", w, h);
            group.bench_function(format!("simd/{id}"), |b| {
                b.iter(|| yuyv_to_rgb_simd(black_box(&src), black_box(&mut dst)));
            });
            group.bench_function(format!("scalar/{id}"), |b| {
                b.iter(|| yuyv_to_rgb_scalar(black_box(&src), black_box(&mut dst)));
            });
        }
        group.finish();
    }

    let mut group = c.benchmark_group("yuyv_to_rgba");
    for &(w, h) in SIZES {
        let pixels = (w as usize) * (h as usize);
        let src_len = pixels * 2;
        let dst_len = pixels * 4;
        let src = vec![128u8; src_len];
        let mut dst = vec![0u8; dst_len];
        group.throughput(Throughput::Bytes(src_len as u64));
        let id = format!("{}x{}", w, h);
        group.bench_function(format!("simd/{id}"), |b| {
            b.iter(|| yuyv_to_rgba_simd(black_box(&src), black_box(&mut dst)));
        });
        group.bench_function(format!("scalar/{id}"), |b| {
            b.iter(|| yuyv_to_rgba_scalar(black_box(&src), black_box(&mut dst)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
