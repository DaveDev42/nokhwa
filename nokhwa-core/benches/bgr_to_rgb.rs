use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use nokhwa_core::simd::bench_exports::{bgr_to_rgb_scalar, bgr_to_rgb_simd};

const SIZES: &[(u32, u32)] = &[(640, 480), (1920, 1080), (3840, 2160)];

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("bgr_to_rgb");
    for &(w, h) in SIZES {
        let n = (w as usize) * (h as usize) * 3;
        let src = vec![128u8; n];
        let mut dst = vec![0u8; n];
        group.throughput(Throughput::Bytes(n as u64));
        let id = format!("{}x{}", w, h);
        group.bench_function(format!("simd/{id}"), |b| {
            b.iter(|| bgr_to_rgb_simd(black_box(&src), black_box(&mut dst)));
        });
        group.bench_function(format!("scalar/{id}"), |b| {
            b.iter(|| bgr_to_rgb_scalar(black_box(&src), black_box(&mut dst)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
