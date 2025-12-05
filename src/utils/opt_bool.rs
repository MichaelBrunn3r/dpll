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
