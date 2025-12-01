use stackvector::StackVec;

// Remove Clause from imports, as it's no longer a public struct we construct manually
use crate::{
    Problem,
    clause::{Clause, Lit},
    constants::MAX_LITS_PER_CLAUSE,
    problem::ProblemBuilder,
};

/// Parses a DIMACS CNF formatted byte array into a Problem instance.
pub fn parse_dimacs_cnf(data: &[u8]) -> Result<Problem, String> {
    let mut iter = ByteArrayIterator::new(data);

    // Skip comments and find the start of the problem line (`p cnf <num_vars> <num_clauses>`)
    if !iter.skip_until(b'p') {
        return Err("Unexpected EOF while searching for problem line".to_string());
    }

    iter.skip_ascii_whitespace();

    // Expect 'cnf'
    if !iter.skip_expected(b"cnf") {
        return Err("Expected problem format 'cnf'".to_string());
    }

    iter.skip_ascii_whitespace();

    // Parse number of variables and clauses
    let num_vars = iter
        .parse_usize()
        .ok_or_else(|| "Expected number of variables".to_string())?;
    iter.skip_ascii_whitespace();
    let num_clauses = iter
        .parse_usize()
        .ok_or_else(|| "Expected number of clauses".to_string())?;

    let mut problem_builder = ProblemBuilder::new(num_vars, num_clauses);

    // Reusable buffer for clause literals
    let mut clause_buffer = Clause(StackVec::new());

    // Parse each clause
    for _ in 0..num_clauses {
        clause_buffer.0.clear();

        // Parse literals until we hit the clause separator 0
        loop {
            iter.skip_ascii_whitespace();

            // Negated literals start with '-' (e.g. -3)
            let is_negated = iter.advance_if(b'-');

            let literal = iter
                .parse_usize()
                .ok_or_else(|| "Expected literal in clause".to_string())?;

            // 0 terminates the clause
            if literal == 0 {
                break;
            }

            if literal > num_vars {
                return Err(format!(
                    "Unexpected variable {}. The problem only declares {} variables.",
                    literal, num_vars
                ));
            }

            // DIMACS variables are 1-indexed; convert to 0-indexed
            let var_idx = literal - 1;

            clause_buffer.0.push(Lit::new(var_idx, !is_negated));
        }

        debug_assert!(
            clause_buffer.len() <= MAX_LITS_PER_CLAUSE,
            "Clause size {} exceeds expected maximum of {} => Allocated on the heap.",
            clause_buffer.len(),
            MAX_LITS_PER_CLAUSE
        );
        problem_builder.add_clause(&mut clause_buffer);
    }

    Ok(problem_builder.build())
}

/// An iterator over a byte array with utility methods for parsing.
/// Performs no bounds checking, because of the assumption that the DIMACS input is well-formed and ends with trailing unused data (after '%').
struct ByteArrayIterator<'a> {
    ptr: *const u8,
    _marker: std::marker::PhantomData<&'a u8>,
}

impl<'a> ByteArrayIterator<'a> {
    fn new(data: &'a [u8]) -> Self {
        let ptr = data.as_ptr();
        ByteArrayIterator {
            ptr,
            _marker: std::marker::PhantomData,
        }
    }

    /// Advances the iterator if the current byte matches the specified byte. No bounds checks.
    fn advance_if(&mut self, byte: u8) -> bool {
        if unsafe { *self.ptr } == byte {
            self.ptr = unsafe { self.ptr.add(1) };
            true
        } else {
            false
        }
    }

    /// Skips bytes until the specified byte is found. No bounds checks.
    fn skip_until(&mut self, byte: u8) -> bool {
        loop {
            let current_byte = unsafe { *self.ptr };
            self.ptr = unsafe { self.ptr.add(1) };
            if current_byte == byte {
                return true;
            }
        }
    }

    /// Skips bytes until the next non-whitespace byte. No bounds checks.
    fn skip_ascii_whitespace(&mut self) {
        while unsafe { (*self.ptr).is_ascii_whitespace() } {
            self.ptr = unsafe { self.ptr.add(1) };
        }
    }

    /// If the next bytes match the expected slice, advances the iterator past them. No bounds checks.
    fn skip_expected(&mut self, expected: &[u8]) -> bool {
        let mut p = self.ptr;
        for &b in expected {
            if unsafe { *p } != b {
                return false;
            }
            p = unsafe { p.add(1) };
        }
        self.ptr = p;
        true
    }

    /// Parses an unsigned integer from the current position. No bounds checks.
    fn parse_usize(&mut self) -> Option<usize> {
        let mut ptr = self.ptr;
        let mut found = false;

        let mut num = 0usize;
        while unsafe { (*ptr).is_ascii_digit() } {
            num = num * 10 + (unsafe { *ptr } - b'0') as usize;
            ptr = unsafe { ptr.add(1) };
            found = true;
        }

        if found {
            self.ptr = ptr;
            Some(num)
        } else {
            None
        }
    }
}
