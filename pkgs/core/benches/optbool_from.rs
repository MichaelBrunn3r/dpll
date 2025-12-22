use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dpll_core::OptBool;

fn bench_from_bool_ifelse(c: &mut Criterion) {
    let mut rng = fastrand::Rng::with_seed(12345);

    c.bench_function("optbool_from_bool_ifelse", |b| {
        b.iter(|| {
            let boolean = black_box(rng.bool());
            let result = black_box(if boolean {
                OptBool::True
            } else {
                OptBool::False
            });

            black_box(result);
        });
    });
}

fn bench_from_bool_branchless(c: &mut Criterion) {
    let mut rng = fastrand::Rng::with_seed(12345);

    c.bench_function("optbool_from_bool_branchless", |b| {
        b.iter(|| {
            let boolean = black_box(rng.bool());
            let val = unsafe { (boolean as u8).unchecked_add(1) };
            let optbool = unsafe { core::mem::transmute::<u8, OptBool>(val) };

            black_box(optbool);
        });
    });
}

fn bench_from_bool_lut(c: &mut Criterion) {
    let mut rng = fastrand::Rng::with_seed(12345);
    let lut = [OptBool::False, OptBool::True];

    c.bench_function("optbool_from_bool_lut", |b| {
        b.iter(|| {
            let boolean = black_box(rng.bool());
            let t = black_box(lut[boolean as usize]);
            black_box(t);
        });
    });
}

criterion_group!(
    benches,
    bench_from_bool_ifelse,
    bench_from_bool_branchless,
    bench_from_bool_lut
);
criterion_main!(benches);
