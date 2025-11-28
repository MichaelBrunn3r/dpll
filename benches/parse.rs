use std::fs;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

fn bench(c: &mut Criterion) {
    let data = fs::read("./uf20-01.cnf").expect("failed to read fixture");

    c.bench_function("parse_dimacs", |b| {
        b.iter(|| {
            let _ = dpll::parser::parse_dimacs(black_box(&data)).unwrap();
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
