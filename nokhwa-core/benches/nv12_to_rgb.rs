use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use nokhwa_core::simd::bench_exports::{nv12_to_rgb_scalar, nv12_to_rgb_simd};

const SIZES: &[(u32, u32)] = &[(640, 480), (1920, 1080), (3840, 2160)];

fn bench(c: &mut Criterion) {
    for &rgba in &[false, true] {
        let pxsize = if rgba { 4 } else { 3 };
        let label = if rgba { "nv12_to_rgba" } else { "nv12_to_rgb" };
        let mut group = c.benchmark_group(label);
        for &(w, h) in SIZES {
            let w = w as usize;
            let h = h as usize;
            let src_len = w * h * 3 / 2;
            let dst_len = w * h * pxsize;
            let src = vec![128u8; src_len];
            let mut dst = vec![0u8; dst_len];
            group.throughput(Throughput::Bytes(src_len as u64));
            let id = format!("{}x{}", w, h);
            group.bench_function(format!("simd/{id}"), |b| {
                b.iter(|| nv12_to_rgb_simd(w, h, black_box(&src), black_box(&mut dst), rgba));
            });
            group.bench_function(format!("scalar/{id}"), |b| {
                b.iter(|| nv12_to_rgb_scalar(w, h, black_box(&src), black_box(&mut dst), rgba));
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
