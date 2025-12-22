use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dpll_core::OptBool;

fn bench_unwrap_or_match(c: &mut Criterion) {
    let mut rng = fastrand::Rng::with_seed(12345);

    c.bench_function("unwrap_or_match", |b| {
        b.iter(|| {
            let opt = black_box(match rng.u8(0..3) {
                0 => OptBool::False,
                1 => OptBool::True,
                _ => OptBool::Unassigned,
            });
            let default = black_box(rng.bool());
            black_box(opt.unwrap_or(default));
        });
    });
}

fn bench_unwrap_or_branchless(c: &mut Criterion) {
    let mut rng = fastrand::Rng::with_seed(12345);

    c.bench_function("unwrap_or_branchless", |b| {
        b.iter(|| {
            // Assume Unassigned = 0b10 for this benchmark
            let val = rng.u8(0..3);
            let opt = match val {
                0 => OptBool::False,
                1 => OptBool::True,
                _ => unsafe { std::mem::transmute(0b10u8) },
            };
            let default = black_box(rng.bool());
            let result = (opt as u8 & 1) != 0 || default;
            black_box(result);
        });
    });
}

criterion_group!(benches, bench_unwrap_or_match, bench_unwrap_or_branchless);
criterion_main!(benches);
