/// A view of a clauses literals.
pub struct ClauseView<'a>(&'a [Lit]);

impl ClauseView<'_> {
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
    pub fn conflicts_with(&self, assignment: &[Option<bool>]) -> bool {
        for &lit in self.0 {
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
    pub fn find_unit_literal(&self, assignment: &[Option<bool>]) -> Option<Lit> {
        let mut unassigned_lit = None;
        let mut unassigned_count = 0;

        for &lit in self.0 {
            let is_pos = lit.is_pos();
            match assignment[lit.var_id()] {
                Some(true) => {
                    if is_pos {
                        return None;
                    }
                } // Clause Satisfied
                Some(false) => {
                    if !is_pos {
                        return None;
                    }
                } // Clause Satisfied
                None => {
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

    /// Checks if the clause is satisfied by the given assignment.
    pub fn satisfied_by(&self, assignment: &[bool]) -> bool {
        for &lit in self.0 {
            if lit.eval_with(assignment[lit.var_id()]) {
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
