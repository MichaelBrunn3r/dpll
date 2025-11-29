use std::fs;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

fn bench_parse_dimacs(c: &mut Criterion) {
    let data = fs::read("./benches/uf20-01.cnf").expect("failed to read fixture");

    c.bench_function("parse_dimacs", |b| {
        b.iter(|| {
            let _ = dpll::parser::parse_dimacs_cnf(black_box(&data)).unwrap();
        })
    });
}

fn bench_solve(c: &mut Criterion) {
    let data = fs::read("./benches/uf20-01.cnf").expect("failed to read fixture");
    let problem = dpll::parser::parse_dimacs_cnf(&data).expect("failed to parse fixture");
    assert_eq!(problem.num_vars, 20);
    assert_eq!(problem.num_clauses(), 91);

    c.bench_function("solve", |b| {
        b.iter(|| {
            let _ = problem.solve();
        })
    });
}

criterion_group!(benches, bench_parse_dimacs, bench_solve);
criterion_main!(benches);
