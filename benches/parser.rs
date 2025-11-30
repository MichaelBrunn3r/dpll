use std::hint::black_box;
use std::{fs, sync::atomic::AtomicBool};

use criterion::{Criterion, criterion_group, criterion_main};
use dpll::dpll::DPLLSolver;

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
    assert_eq!(problem.clauses.len(), 91);
    let mut assignment_buffer = vec![None; problem.num_vars];

    c.bench_function("solve", |b| {
        b.iter(|| {
            assignment_buffer.fill(None);
            let mut solver =
                DPLLSolver::with_assignment(black_box(&problem), &mut assignment_buffer);
            let _ = solver.solve(&AtomicBool::new(false));
        })
    });
}

criterion_group!(benches, bench_parse_dimacs, bench_solve);
criterion_main!(benches);
