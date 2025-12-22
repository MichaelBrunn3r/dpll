/// A cursor to traverse a slice. Assumes that there is always a matching item in the slice.
#[derive(Debug, Default, Clone, Copy)]
pub struct NonExhaustingCursor {
    idx: usize,
}

impl NonExhaustingCursor {
    #[inline(always)]
    pub fn new() -> Self {
        Self { idx: 0 }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.idx = 0;
    }

    /// Finds the next item in the slice that matches the given predicate.
    /// # Safety
    /// This function assumes that there is always a matching item in the slice.
    #[inline(always)]
    pub fn next_match<'a, T, P>(&mut self, slice: &'a [T], mut predicate: P) -> &'a T
    where
        P: FnMut(&T) -> bool,
    {
        loop {
            debug_assert!(
                self.idx < slice.len(),
                "NonExhaustingCursor invariant broken: No matching item found."
            );

            let item = unsafe { slice.get_unchecked(self.idx) };
            self.idx += 1;

            if predicate(item) {
                return item;
            }
        }
    }
}

#[macro_export]
macro_rules! dprintln {
    ($($t:tt)*) => {
        #[cfg(debug_assertions)]
        println!($($t)*);
    };
}
