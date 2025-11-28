// Remove Clause from imports, as it's no longer a public struct we construct manually
use crate::Problem;

/// Parses DIMACS CNF format from a byte slice.
pub fn parse_dimacs(data: &[u8]) -> Result<Problem, String> {
    let mut iter = ByteArrayIterator::new(data);

    if !iter.skip_until(b'p') {
        return Err("Unexpected EOF while searching for problem line".to_string());
    }

    iter.skip_ascii_whitespace();

    if !iter.skip_expected(b"cnf") {
        return Err("Expected problem format 'cnf'".to_string());
    }

    iter.skip_ascii_whitespace();

    let num_vars = iter
        .parse_usize()
        .ok_or_else(|| "Expected number of variables".to_string())?;

    iter.skip_ascii_whitespace();

    let num_clauses = iter
        .parse_usize()
        .ok_or_else(|| "Expected number of clauses".to_string())?;

    // CHANGE: Problem::new now only requires num_vars.
    // The internal vectors grow as needed.
    let mut problem = Problem::new(num_vars);

    iter.skip_ascii_whitespace();

    // CHANGE: Reusable buffer to store literals for the current clause
    // to avoid allocation on every loop iteration.
    // Tuple is (variable_index, is_positive)
    let mut clause_buffer: Vec<(usize, bool)> = Vec::with_capacity(8);

    // Populate the Clauses
    for _ in 0..num_clauses {
        clause_buffer.clear();

        loop {
            iter.skip_ascii_whitespace();

            // Check for negation
            let is_negated = if iter.peek() == Some(b'-') {
                iter.position += 1;
                true
            } else {
                false
            };

            let literal = iter
                .parse_usize()
                .ok_or_else(|| "Expected literal in clause".to_string())?;

            // 0 terminates the clause
            if literal == 0 {
                break;
            }

            // DIMACS variables are 1-indexed; convert to 0-indexed
            let var_idx = literal - 1;

            if var_idx >= num_vars {
                return Err(format!(
                    "Variable {} exceeds declared num_vars {}",
                    literal, num_vars
                ));
            }

            // CHANGE: Instead of calculating bitset index, we push the tuple
            // (variable, is_positive).
            // if is_negated is true, is_positive is false.
            clause_buffer.push((var_idx, !is_negated));
        }

        // CHANGE: Pass the slice to the problem to pack into the flat database
        problem.add_clause(&clause_buffer);
    }

    Ok(problem)
}

// -------------------------------------------------------------
// The Iterator logic below remains completely unchanged
// -------------------------------------------------------------

/// An iterator over a byte array with utility methods for parsing.
struct ByteArrayIterator<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> ByteArrayIterator<'a> {
    fn new(data: &'a [u8]) -> Self {
        ByteArrayIterator { data, position: 0 }
    }

    fn peek(&self) -> Option<u8> {
        if self.position < self.data.len() {
            Some(self.data[self.position])
        } else {
            None
        }
    }

    fn skip_until(&mut self, byte: u8) -> bool {
        while self.position < self.data.len() {
            let current_byte = self.data[self.position];
            self.position += 1;
            if current_byte == byte {
                return true;
            }
        }
        false
    }

    fn skip_ascii_whitespace(&mut self) {
        while self.position < self.data.len() && self.data[self.position].is_ascii_whitespace() {
            self.position += 1;
        }
    }

    fn skip_expected(&mut self, expected: &[u8]) -> bool {
        if self.position + expected.len() > self.data.len() {
            return false;
        }
        if &self.data[self.position..self.position + expected.len()] != expected {
            return false;
        }
        self.position += expected.len();
        true
    }

    fn parse_usize(&mut self) -> Option<usize> {
        let mut num = 0usize;
        let start_pos = self.position;

        while self.position < self.data.len() && self.data[self.position].is_ascii_digit() {
            num = num * 10 + (self.data[self.position] - b'0') as usize;
            self.position += 1;
        }

        if self.position == start_pos {
            None
        } else {
            Some(num)
        }
    }
}
