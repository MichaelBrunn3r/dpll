use crate::{
    constants::MAX_LITS_PER_CLAUSE, partial_assignment::PartialAssignment, utils::opt_bool::OptBool,
};
use stackvector::StackVec;
use std::ops::{Deref, DerefMut};

/// A view of a clauses literals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause(pub StackVec<[Lit; MAX_LITS_PER_CLAUSE]>);

impl Clause {
    /// Checks if a clause is a tautology (contains both a literal and its negation).
    /// Assumes the clause is sorted and contains unique literals.
    pub fn is_tautology(&self) -> bool {
        for i in 0..self.0.len().saturating_sub(1) {
            if self.0[i].var() == self.0[i + 1].var() {
                return true;
            }
        }
        false
    }

    /// Checks if the clause is satisfied by the given assignment.
    pub fn is_satisfied_by(&self, assignment: &[bool]) -> bool {
        for &lit in &self.0 {
            if lit.eval_with(assignment[lit.var()]) {
                return true;
            }
        }
        false
    }

    /// Checks if the clause is unsatisfied by the given partial assignment.
    pub fn is_unsatisfied_by_partial(&self, part_assignment: &[OptBool]) -> bool {
        self.0.iter().all(|&lit| {
            let var_state = PartialAssignment::get_unchecked_from(part_assignment, lit.var());
            var_state.is_bool(!lit.is_pos())
        })
    }

    /// Checks if the clause is satisfied by the given partial assignment.
    pub fn is_satisfied_by_partial(&self, part_assignment: &PartialAssignment) -> bool {
        self.0.iter().any(|&lit| {
            let var_state = part_assignment.get_unchecked(lit.var());
            var_state.is_bool(lit.is_pos())
        })
    }

    /// Evaluates the clause under the given partial assignment.
    pub fn eval_with_partial(&self, part_assignment: &PartialAssignment) -> ClauseState {
        let mut unassigned_count = 0usize;
        let mut unit_lit = Lit::INVALID;

        for &lit in &self.0 {
            let var_state = part_assignment.get_unchecked(lit.var());

            if var_state.is_bool(lit.is_pos()) {
                return ClauseState::Satisfied;
            } else if var_state.is_none() {
                unassigned_count += 1;
                unit_lit = lit;
            }
        }

        match unassigned_count {
            0 => ClauseState::Unsatisfied, // All assigned false
            1 => {
                // Exactly one unassigned, others false
                debug_assert!(
                    unit_lit != Lit::INVALID,
                    "Unit literal should have been set."
                );
                ClauseState::Unit(unit_lit)
            }
            _ => ClauseState::Undecided(unassigned_count), // More than one unassigned
        }
    }
}

impl Deref for Clause {
    type Target = StackVec<[Lit; MAX_LITS_PER_CLAUSE]>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Clause {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub enum ClauseState {
    /// Under the current assignment, the clause is satisfied (at least one literal evaluates to true).
    Satisfied,
    /// Under the current assignment, the clause is unsatisfied (all literals evaluate to false).
    Unsatisfied,
    /// Under the current assignment, the clause is a unit clause (exactly one unassigned literal, others evaluate to false).
    Unit(Lit),
    /// Under the current assignment, the clause is undecided (more than one unassigned literal).
    Undecided(usize),
}

// --------------------------------
// Literal
// --------------------------------

pub type VariableId = usize;

/// A propositional logic literal, i.e. a variable or its negation.
///
/// E.g. `x` or `¬x`, where `x` is a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Lit(pub u32);

impl Lit {
    const INVALID: Lit = Lit(u32::MAX);

    #[inline(always)]
    pub fn new(var: usize, is_pos: bool) -> Self {
        Lit((var as u32) << 1 | (!is_pos as u32))
    }

    /// Returns the variable ID (0-based) of the literal.
    #[inline(always)]
    pub fn var(self) -> usize {
        (self.0 >> 1) as usize
    }

    /// Returns true if the variable is positive.
    #[inline(always)]
    pub fn is_pos(self) -> bool {
        (self.0 & 1) == 0
    }

    /// Returns true if the variable is negative (negated).
    #[inline(always)]
    pub fn is_neg(self) -> bool {
        (self.0 & 1) == 1
    }

    /// Returns the same literal but negative.
    #[inline(always)]
    pub fn negated(self) -> Self {
        Lit::new(self.var(), false)
    }

    /// Returns the inverse (negation) of the literal.
    #[inline(always)]
    pub fn inverted(self) -> Self {
        Lit(self.0 ^ 1)
    }

    /// Evaluates the literal given a boolean value for its variable.
    ///
    /// E.g. assigning the variable `x=true` makes the literal `x` evaluate to `true` and `¬x` evaluate to `false`.
    pub fn eval_with(self, value: bool) -> bool {
        self.is_neg() ^ value
    }
}

impl From<i32> for Lit {
    fn from(value: i32) -> Self {
        let var = value.abs() as usize - 1;
        let is_pos = value > 0;
        Lit::new(var, is_pos)
    }
}

#[cfg(test)]
mod tests {

    use std::cmp::Ordering;

    use super::*;

    #[test]
    fn test_lit_order() {
        let cases: Vec<(i32, i32, Ordering)> = vec![
            (1, 2, Ordering::Less),
            (2, 1, Ordering::Greater),
            (1, -1, Ordering::Less),
            (-1, 1, Ordering::Greater),
            (-2, -1, Ordering::Greater),
            (-1, -2, Ordering::Less),
            (3, 3, Ordering::Equal),
            (-3, -3, Ordering::Equal),
        ];

        for (a, b, expected) in cases {
            let lit_a = Lit::from(a);
            let lit_b = Lit::from(b);
            assert_eq!(lit_a.cmp(&lit_b), expected, "Comparing {} and {}", a, b);
        }
    }

    #[test]
    fn test_is_tautology() {
        let cases: Vec<(Vec<i32>, bool)> = vec![
            (vec![1, -1], true),
            (vec![-2, 1, 1, 1, 2], true),
            (vec![-3, 3, 4], true),
            (vec![1, 2, 3], false),
            (vec![-1, -2, -3], false),
            (vec![1, -2, 3], false),
            (vec![], false),
        ];

        for (clause_ints, expected) in cases {
            let mut clause: Clause = Clause(clause_ints.iter().map(|&x| Lit::from(x)).collect());
            clause.sort_unstable();
            clause.dedup();

            assert_eq!(
                clause.is_tautology(),
                expected,
                "Tautology check failed for clause {:?}",
                clause_ints
            );
        }
    }
}
