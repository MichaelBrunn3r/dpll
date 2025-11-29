use crate::LBool;

/// A view of a clauses literals.
pub struct ClauseView<'a>(&'a [Lit]);

impl ClauseView<'_> {
    /// Checks if a clause is a tautology (contains both a literal and its negation).
    /// Assumes the clause is sorted and contains unique literals.
    pub fn is_tautology(&self) -> bool {
        for i in 0..self.0.len().saturating_sub(1) {
            if self.0[i].var_index() == self.0[i + 1].var_index() {
                return true;
            }
        }
        false
    }

    /// Checks if the clause conflicts with the given assignment.
    /// A conflict occurs if all literals in the clause evaluate to false.
    pub fn conflicts_with(&self, assignment: &[LBool]) -> bool {
        // Conflict if ALL literals evaluate to false
        for &lit in self.0 {
            match assignment[lit.var_index()] {
                LBool::UNDEF => return false, // Not a conflict yet
                LBool::TRUE => {
                    if lit.is_pos() {
                        return false;
                    }
                }
                LBool::FALSE => {
                    if !lit.is_pos() {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// For a given assignment, finds a unit literal in the clause if it exists.
    pub fn find_unit_literal(&self, assignment: &[LBool]) -> Option<Lit> {
        let mut unassigned_lit = None;
        let mut unassigned_count = 0;

        for &lit in self.0 {
            let is_pos = lit.is_pos();
            match assignment[lit.var_index()] {
                LBool::TRUE => {
                    if is_pos {
                        return None;
                    }
                } // Clause Satisfied
                LBool::FALSE => {
                    if !is_pos {
                        return None;
                    }
                } // Clause Satisfied
                LBool::UNDEF => {
                    unassigned_count += 1;
                    unassigned_lit = Some(lit);
                }
            }
        }

        if unassigned_count == 1 {
            unassigned_lit
        } else {
            None
        }
    }

    /// Checks if the clause is satisfied by the given boolean assignment.
    pub fn satisfied_by(&self, assignment: &[bool]) -> bool {
        for &lit in self.0 {
            let is_pos = lit.is_pos();
            let var_value = assignment[lit.var_index()];
            if (is_pos && var_value) || (!is_pos && !var_value) {
                return true;
            }
        }
        false
    }
}

impl<'a> From<&'a [Lit]> for ClauseView<'a> {
    fn from(slice: &'a [Lit]) -> Self {
        ClauseView(slice)
    }
}

impl<'a> From<&'a Vec<Lit>> for ClauseView<'a> {
    fn from(vec: &'a Vec<Lit>) -> Self {
        ClauseView(vec.as_slice())
    }
}

impl<'a> IntoIterator for ClauseView<'a> {
    type Item = &'a Lit;
    type IntoIter = std::slice::Iter<'a, Lit>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

// --------------------------------
// Literal
// --------------------------------

// Represents a literal as an integer.
// Even numbers are positive literals (v), Odd numbers are negative (!v).
// Var 0 -> Lit 0 (pos), Lit 1 (neg)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lit(pub u32);

impl Lit {
    #[inline]
    pub fn new(var: usize, is_pos: bool) -> Self {
        Lit((var as u32) << 1 | (!is_pos as u32))
    }

    /// Returns the variable index (0-based).
    pub fn var_index(&self) -> usize {
        (self.0 >> 1) as usize
    }

    /// Returns true if the literal is positive.
    pub fn is_pos(&self) -> bool {
        (self.0 & 1) == 0
    }

    /// Returns the negation of the literal.
    pub fn negated(&self) -> Self {
        Lit(self.0 ^ 1)
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
            write!(f, "{}", self.var_index() + 1)
        } else {
            write!(f, "¬{}", self.var_index() + 1)
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
            let mut clause: Vec<Lit> = clause_ints.iter().map(|&x| Lit::from(x)).collect();
            clause.sort_unstable();
            clause.dedup();

            assert_eq!(
                ClauseView::from(&clause).is_tautology(),
                expected,
                "Tautology check failed for clause {:?}",
                clause_ints
            );
        }
    }
}
