use std::time::Duration;

pub mod opt_bool;

pub fn human_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs_f64();
    if total_secs < 0.000_001 {
        format!("{:.1}ns", total_secs * 1_000_000_000.0)
    } else if total_secs < 0.001 {
        format!("{:.1}µs", total_secs * 1_000_000.0)
    } else if total_secs < 1.0 {
        format!("{:.1}ms", total_secs * 1000.0)
    } else {
        format!("{:.1}s", total_secs)
    }
}

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

#[macro_export]
macro_rules! record_time {
    ($durations:expr, $block:block) => {{
        let start = std::time::Instant::now();
        let result = { $block };
        let duration = start.elapsed();
        $durations.push(duration);
        result
    }};
}

#[macro_export]
macro_rules! measure_time {
    ($block:block) => {{
        let start = std::time::Instant::now();
        {
            $block
        };
        let duration = start.elapsed();
        duration
    }};
}

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

/// A utility that manages a tiered, capped backoff strategy for non-blocking polls.
pub struct Backoff {
    num_spins: usize,
    spin_limit: usize,
    num_yields: usize,
    yield_limit: usize,
    initial_sleep: Duration,
    current_sleep: Duration,
    sleep_limit: Duration,
    sleep_multiplier: f32,
}

/// A tiered backoff strategy for non-blocking polls.
/// The strategy consists of three tiers:
/// 1. Spinning: The thread will spin for a fixed number of iterations.
/// 2. Yielding: The thread will yield to the scheduler for a fixed number of iterations.
/// 3. Sleeping: The thread will sleep for an exponentially increasing duration, capped at a maximum.
impl Backoff {
    /// Creates a new Backoff instance with the specified maximum sleep duration.
    pub fn new(
        spin_limit: usize,
        yield_limit: usize,
        initial_sleep: Duration,
        sleep_limit: Duration,
        sleep_multiplier: f32,
    ) -> Self {
        Backoff {
            num_spins: 0,
            spin_limit,
            num_yields: 0,
            yield_limit,
            initial_sleep,
            current_sleep: initial_sleep,
            sleep_limit,
            sleep_multiplier,
        }
    }

    pub fn wait(&mut self) {
        if self.num_spins < self.spin_limit {
            std::hint::spin_loop();
            self.num_spins += 1;
        } else if self.num_yields < self.yield_limit {
            std::thread::yield_now();
            self.num_yields += 1;
        } else {
            std::thread::sleep(self.current_sleep);

            let next_sleep = self.current_sleep.as_secs_f32() * self.sleep_multiplier;

            self.current_sleep = Duration::from_secs_f32(next_sleep).min(self.sleep_limit);
        }
    }

    pub fn reset(&mut self) {
        self.num_spins = 0;
        self.num_yields = 0;
        self.current_sleep = self.initial_sleep;
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
