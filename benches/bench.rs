use std::fs;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dpll::{dpll::DPLLSolver, utils::opt_bool::OptBool};

fn bench_parse_dimacs(c: &mut Criterion) {
    let data = fs::read("./benches/uf100-01.cnf").expect("failed to read fixture");

    c.bench_function("parse_dimacs", |b| {
        b.iter(|| {
            let _ = dpll::parser::parse_dimacs_cnf(black_box(&data)).unwrap();
        })
    });
}

fn bench_solve(c: &mut Criterion) {
    let data = fs::read("./benches/uf100-01.cnf").expect("failed to read fixture");
    let problem = dpll::parser::parse_dimacs_cnf(&data).expect("failed to parse fixture");
    let mut assignment_buffer = vec![OptBool::Unassigned; problem.num_vars];

    c.bench_function("solve", |b| {
        b.iter(|| {
            assignment_buffer.fill(OptBool::Unassigned);
            let mut solver =
                DPLLSolver::with_assignment(black_box(&problem), &mut assignment_buffer);
            let _ = solver.solve();
        })
    });
}

criterion_group!(benches, bench_parse_dimacs, bench_solve);
criterion_main!(benches);
