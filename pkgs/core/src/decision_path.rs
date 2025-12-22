use crate::{lit::Lit, opt_bool::OptBool};

/// A sequence of variable assignment decisions made during search.
/// Can be stolen by idle workers and helps them reconstruct search states.
#[derive(Debug)]
pub struct DecisionPath(pub Vec<Lit>);

impl DecisionPath {
    pub fn to_assignment(&self, num_vars: usize) -> Vec<OptBool> {
        let mut assignment = vec![OptBool::Unassigned; num_vars];
        for lit in &self.0 {
            assignment[lit.var() as usize] = OptBool::from(lit.is_pos());
        }
        assignment
    }
}

impl From<Vec<Lit>> for DecisionPath {
    fn from(decisions: Vec<Lit>) -> Self {
        Self(decisions)
    }
}

impl From<Vec<i32>> for DecisionPath {
    fn from(value: Vec<i32>) -> Self {
        DecisionPath(value.iter().map(|&x| Lit::from(x)).collect())
    }
}
