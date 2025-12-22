use std::fs;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dpll_core::{DPLLSolver, DecisionPath, parse_dimacs_cnf};

fn bench_parse_dimacs(c: &mut Criterion) {
    let data = fs::read("./benches/uf100-01.cnf").expect("failed to read fixture");

    c.bench_function("parse_dimacs", |b| {
        b.iter(|| {
            let _ = parse_dimacs_cnf(black_box(&data)).unwrap();
        })
    });
}

fn bench_solve(c: &mut Criterion) {
    let data = fs::read("./benches/uf100-01.cnf").expect("failed to read fixture");
    let problem = parse_dimacs_cnf(&data).expect("failed to parse fixture");
    let initial_decisions = DecisionPath(vec![]);

    c.bench_function("solve", |b| {
        b.iter(|| {
            let mut solver = DPLLSolver::with_decisions(black_box(&problem), &initial_decisions);
            let _ = solver.solve();
        })
    });
}

criterion_group!(benches, bench_parse_dimacs, bench_solve);
criterion_main!(benches);
