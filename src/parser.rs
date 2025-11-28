use fixedbitset::FixedBitSet;

#[derive(Debug, Clone)]
pub struct Problem {
    pub num_vars: usize,
    pub num_clauses: usize,
}

#[derive(Debug, Clone)]
pub struct Clause {
    pub pos: FixedBitSet,
    pub neg: FixedBitSet,
}

impl Clause {
    fn new(num_vars: usize) -> Self {
        Clause {
            pos: FixedBitSet::with_capacity(num_vars),
            neg: FixedBitSet::with_capacity(num_vars),
        }
    }
}

/// Parse a DIMACS CNF formatted byte array into a Problem and its Clauses
pub fn parse_dimacs(data: &[u8]) -> Result<(Problem, Vec<Clause>), String> {
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

    let problem = Problem {
        num_vars: num_vars as usize,
        num_clauses: num_clauses as usize,
    };

    iter.skip_ascii_whitespace();

    let mut clauses = Vec::<Clause>::with_capacity(problem.num_clauses);
    while clauses.len() < problem.num_clauses {
        let mut clause = Clause::new(problem.num_vars);

        loop {
            iter.skip_ascii_whitespace();

            let is_negated = data[iter.position] == b'-';
            if is_negated {
                iter.position += 1;
            }

            let literal = iter
                .parse_usize()
                .ok_or_else(|| "Expected literal in clause".to_string())?;

            if literal == 0 {
                break;
            } else if is_negated {
                clause.neg.insert(literal - 1);
            } else {
                clause.pos.insert(literal - 1);
            }
        }
        clauses.push(clause);
    }

    Ok((problem, clauses))
}

impl std::fmt::Display for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;

        for i in self.pos.ones() {
            if !first {
                write!(f, " ∨ ")?;
            }
            write!(f, "{}", i + 1)?;
            first = false;
        }

        for i in self.neg.ones() {
            if !first {
                write!(f, " ∨ ")?;
            }
            write!(f, "¬{}", i + 1)?;
            first = false;
        }

        Ok(())
    }
}

/// A custom iterator over a byte array for parsing
struct ByteArrayIterator<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> ByteArrayIterator<'a> {
    fn new(data: &'a [u8]) -> Self {
        ByteArrayIterator { data, position: 0 }
    }

    /// Skip bytes until (and including) the specified byte is found
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

    /// Skip ASCII whitespace characters
    fn skip_ascii_whitespace(&mut self) {
        while self.position < self.data.len() && self.data[self.position].is_ascii_whitespace() {
            self.position += 1;
        }
    }

    /// Skip the expected byte sequence. Returns true if the sequence was found and skipped, false otherwise.
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

    /// Parse an unsigned integer from the current position
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
