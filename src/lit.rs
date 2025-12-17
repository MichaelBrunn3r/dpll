pub type VariableId = usize;

/// A propositional logic literal, i.e. a variable or its negation.
///
/// E.g. `x` or `¬x`, where `x` is a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Lit(pub u32);

impl Lit {
    pub const INVALID: Lit = Lit(u32::MAX);

    #[inline(always)]
    pub fn new(var: usize, is_pos: bool) -> Self {
        Lit((var as u32) << 1 | (!is_pos as u32))
    }

    /// Returns the variable ID (0-based) of the literal.
    #[inline(always)]
    pub fn var(self) -> VariableId {
        (self.0 >> 1) as VariableId
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
