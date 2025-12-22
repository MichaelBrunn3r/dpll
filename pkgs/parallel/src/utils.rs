use std::num::NonZeroUsize;

/// Converts a code block into a generator.
/// A generator is a function that can yield values, paused and be resumed.
/// It behaves very similarly to an iterator.
#[macro_export]
macro_rules! generator {
    ($code:expr) => {
        std::iter::from_coroutine(
            #[coroutine]
            $code,
        )
    };
}

pub const PRIMES: [usize; 60] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281,
];

/// Finds a prime number that is coprime to the given number.
/// The search starts at the nth prime (start_offset).
/// If no coprime prime is found, returns 1.
pub fn find_coprime_to(start_offset: usize, num: usize) -> usize {
    if num <= 1 {
        return 1;
    }

    for i in 0..PRIMES.len() {
        let prime: usize = PRIMES[(start_offset + i) % PRIMES.len()];
        let is_coprime = num % prime != 0;
        if is_coprime {
            return prime;
        }
    }
    1
}

pub trait NonZeroUsizeExt {
    fn ilog2_nz_clamped(&self) -> NonZeroUsize;
}

impl NonZeroUsizeExt for NonZeroUsize {
    fn ilog2_nz_clamped(&self) -> NonZeroUsize {
        let log = self.get().ilog2() as usize;
        NonZeroUsize::new(log.max(1)).unwrap()
    }
}

pub trait VecExt<T> {
    fn ensure_capacity(&mut self, min: usize);
}

impl<T> VecExt<T> for Vec<T> {
    fn ensure_capacity(&mut self, min: usize) {
        if self.capacity() < min {
            self.reserve(min - self.capacity());
        }
    }
}
