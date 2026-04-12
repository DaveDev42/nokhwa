use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use nokhwa_core::simd::bench_exports::{rgb_to_luma_scalar, rgb_to_luma_simd};

const SIZES: &[(u32, u32)] = &[(640, 480), (1920, 1080), (3840, 2160)];

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("rgb_to_luma");
    for &(w, h) in SIZES {
        let pixels = (w as usize) * (h as usize);
        let src = vec![128u8; pixels * 3];
        let mut dst = vec![0u8; pixels];
        group.throughput(Throughput::Bytes((pixels * 3) as u64));
        let id = format!("{}x{}", w, h);
        group.bench_function(format!("simd/{id}"), |b| {
            b.iter(|| rgb_to_luma_simd(black_box(&src), black_box(&mut dst)));
        });
        group.bench_function(format!("scalar/{id}"), |b| {
            b.iter(|| rgb_to_luma_scalar(black_box(&src), black_box(&mut dst)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
