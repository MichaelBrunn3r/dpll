use std::time::Duration;

/// Maximum number of literals in a clause.
pub const MAX_LITS_PER_CLAUSE: usize = 16;
/// Maximum number of clauses a variable can appear in.
pub const MAX_CLAUSES_PER_VAR: usize = 64;
/// Maximum number of clauses a literal can appear in.
pub const MAX_CLAUSES_PER_LIT: usize = 32;
/// Maximum number of literals that can become falsified during unit propagation.
pub const MAX_FALSIFIED_LITS: usize = 128;
/// Minimal runtime after which to show a progress bar instead of simple log messages.
pub const PROGRESS_BAR_THRESHOLD: Duration = Duration::from_millis(500);
