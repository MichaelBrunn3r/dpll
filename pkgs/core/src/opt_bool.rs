/// A three-state boolean: true, false, or unassigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OptBool {
    False = 0b00,
    True = 0b01,
    Unassigned = 0b10,
}

impl OptBool {
    #[inline(always)]
    pub fn is_some(self) -> bool {
        self != OptBool::Unassigned
    }

    #[inline(always)]
    pub fn is_none(self) -> bool {
        self == OptBool::Unassigned
    }

    #[inline(always)]
    pub fn is_true(self) -> bool {
        self == OptBool::True
    }

    #[inline(always)]
    pub fn is_false(self) -> bool {
        self == OptBool::False
    }

    #[inline(always)]
    pub fn is_bool(self, val: bool) -> bool {
        (self as u8) == (val as u8)
    }

    #[inline(always)]
    pub fn unwrap(self) -> bool {
        debug_assert!(self.is_some(), "Called unwrap on an unassigned OptBool.");
        self.is_true()
    }

    #[inline(always)]
    pub fn unwrap_or(self, default: bool) -> bool {
        (self as u8 & 1) != 0 || default
    }
}

impl From<bool> for OptBool {
    #[inline(always)]
    fn from(b: bool) -> Self {
        unsafe { std::mem::transmute(b as u8) }
    }
}

pub struct OptBoolVec {
    words: Vec<usize>,
    len: usize,
}

impl OptBoolVec {
    const BITS_PER_ITEM: usize = 2;
    const ITEMS_PER_WORD: usize = (std::mem::size_of::<usize>() * 8) / Self::BITS_PER_ITEM;
    const MASK_INDEX: usize = Self::ITEMS_PER_WORD - 1;
    const SHIFT: u32 = (Self::ITEMS_PER_WORD as u32).trailing_zeros();

    pub fn new() -> Self {
        Self {
            words: Vec::new(),
            len: 0,
        }
    }

    pub fn new_unassigned(len: usize) -> Self {
        const UNASSIGNED_WORD: usize = (usize::MAX / 3) * 2;
        let num_words = (len + Self::ITEMS_PER_WORD - 1) / Self::ITEMS_PER_WORD;
        Self {
            words: vec![UNASSIGNED_WORD; num_words],
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get_unchecked(&self, index: usize) -> OptBool {
        debug_assert!(index < self.len, "index {} >= len {}", index, self.len);

        let word_idx = index >> Self::SHIFT;
        let bit_offset = (index & Self::MASK_INDEX) * Self::BITS_PER_ITEM;

        let word = unsafe { *self.words.get_unchecked(word_idx) };

        let val = (word >> bit_offset) & 0b11;
        unsafe { std::mem::transmute(val as u8) }
    }

    pub fn set_unchecked(&mut self, index: usize, value: OptBool) {
        debug_assert!(index < self.len, "index {} >= len {}", index, self.len);

        let word_idx = index >> Self::SHIFT;
        let bit_offset = (index & Self::MASK_INDEX) * Self::BITS_PER_ITEM;

        let mask = !(0b11 << bit_offset);
        let val_shifted = (value as usize) << bit_offset;

        // Safety: Bounds checked above.
        let word_ref = unsafe { self.words.get_unchecked_mut(word_idx) };
        *word_ref = (*word_ref & mask) | val_shifted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_unchecked() {
        let mut obv = OptBoolVec::new_unassigned(3);

        obv.set_unchecked(0, OptBool::True);
        obv.set_unchecked(1, OptBool::False);
        obv.set_unchecked(2, OptBool::True);

        assert_eq!(obv.get_unchecked(0), OptBool::True);
        assert_eq!(obv.get_unchecked(1), OptBool::False);
        assert_eq!(obv.get_unchecked(2), OptBool::True);

        obv.set_unchecked(1, OptBool::Unassigned);
        obv.set_unchecked(2, OptBool::False);

        assert_eq!(obv.get_unchecked(0), OptBool::True);
        assert_eq!(obv.get_unchecked(1), OptBool::Unassigned);
        assert_eq!(obv.get_unchecked(2), OptBool::False);
    }
}
