use std::ops::{Deref, DerefMut};

use crate::PartialAssignment;

/// A view of a clauses literals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause(pub Vec<Lit>);

impl Clause {
    /// Checks if a clause is a tautology (contains both a literal and its negation).
    /// Assumes the clause is sorted and contains unique literals.
    pub fn is_tautology(&self) -> bool {
        for i in 0..self.0.len().saturating_sub(1) {
            if self.0[i].var_id() == self.0[i + 1].var_id() {
                return true;
            }
        }
        false
    }

    /// Checks if the clause conflicts with the given partial assignment.
    /// A conflict occurs if all literals in the clause evaluate to false.
    pub fn conflicts_with(&self, assignment: &PartialAssignment) -> bool {
        for &lit in &self.0 {
            match assignment[lit.var_id()] {
                None => return false, // Not a conflict yet
                Some(true) => {
                    if lit.is_pos() {
                        return false;
                    }
                }
                Some(false) => {
                    if lit.is_neg() {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// For a given partial assignment, finds a unit literal in the clause if it exists.
    pub fn find_unit_literal(&self, assignment: &PartialAssignment) -> Option<Lit> {
        let mut unassigned_count = 0usize;
        let mut unit_lit = None;

        for &lit in &self.0 {
            let is_pos = lit.is_pos();
            match assignment[lit.var_id()] {
                Some(true) => {
                    if is_pos {
                        return None;
                    }
                }
                Some(false) => {
                    if !is_pos {
                        return None;
                    }
                }
                None => {
                    if unassigned_count > 0 {
                        return None;
                    }
                    unassigned_count += 1;
                    unit_lit = Some(lit);
                }
            }
        }

        unit_lit
    }

    /// Checks if the clause is satisfied by the given assignment.
    pub fn is_satisfied_by(&self, assignment: &[bool]) -> bool {
        for &lit in &self.0 {
            if lit.eval_with(assignment[lit.var_id()]) {
                return true;
            }
        }
        false
    }

    /// Evaluates the clause under the given partial assignment.
    pub fn eval_with(&self, assignment: &PartialAssignment) -> ClauseState {
        let mut unassigned_count = 0usize;
        let mut unit_lit = None;

        for &lit in &self.0 {
            if let Some(val) = assignment[lit.var_id()] {
                if lit.eval_with(val) {
                    return ClauseState::Satisfied;
                }
            } else {
                unassigned_count += 1;
                unit_lit = Some(lit);
            }
        }

        match unassigned_count {
            0 => ClauseState::Unsatisfied,                 // All assigned false
            1 => ClauseState::Unit(unit_lit.unwrap()),     // Exactly one unassigned, others false
            _ => ClauseState::Undecided(unassigned_count), // More than one unassigned
        }
    }
}

impl Deref for Clause {
    type Target = Vec<Lit>;
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

/// A propositional logic literal, i.e. a variable or its negation.
///
/// E.g. `x` or `¬x`, where `x` is a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lit(pub u32);

impl Lit {
    #[inline]
    pub fn new(var: usize, is_pos: bool) -> Self {
        Lit((var as u32) << 1 | (!is_pos as u32))
    }

    /// Returns the variable ID (0-based) of the literal.
    pub fn var_id(&self) -> usize {
        (self.0 >> 1) as usize
    }

    /// Returns true if the variable is not negated.
    pub fn is_pos(&self) -> bool {
        (self.0 & 1) == 0
    }

    /// Returns true if the variable is negated.
    pub fn is_neg(&self) -> bool {
        (self.0 & 1) == 1
    }

    /// Returns the negated version of this literal.
    pub fn negated(&self) -> Self {
        Lit(self.0 ^ 1)
    }

    pub fn negated_var_id(var_id: usize) -> usize {
        (var_id << 1) | 1
    }

    /// Evaluates the literal given a boolean value for its variable.
    ///
    /// E.g. assigning the variable `x=true` makes the literal `x` evaluate to `true` and `¬x` evaluate to `false`.
    pub fn eval_with(&self, value: bool) -> bool {
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

impl std::fmt::Display for Lit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_pos() {
            write!(f, "{}", self.var_id() + 1)
        } else {
            write!(f, "¬{}", self.var_id() + 1)
        }
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
