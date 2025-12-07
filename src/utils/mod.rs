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
    spins: usize,
    current_sleep: Duration,
    max_sleep: Duration,
}

/// A tiered backoff strategy for non-blocking polls.
/// The strategy consists of three tiers:
/// 1. Spinning: The thread will spin for a fixed number of iterations.
/// 2. Yielding: The thread will yield to the scheduler for a fixed number of iterations.
/// 3. Sleeping: The thread will sleep for an exponentially increasing duration, capped at a maximum.
impl Backoff {
    /// Maximum number of spin iterations before yielding.
    const SPIN_LIMIT: usize = 100;
    /// Maximum number of yields before sleeping.
    const YIELD_LIMIT: usize = 200;
    /// Initial sleep duration.
    const INITIAL_SLEEP: Duration = Duration::from_micros(50);
    /// Multiplier for exponential backoff.
    const SLEEP_MULTIPLIER: f32 = 1.5;

    /// Creates a new Backoff instance with the specified maximum sleep duration.
    pub fn new(max_sleep: Duration) -> Self {
        Backoff {
            spins: 0,
            current_sleep: Self::INITIAL_SLEEP,
            max_sleep,
        }
    }

    pub fn wait(&mut self) {
        if self.spins < Self::SPIN_LIMIT {
            std::hint::spin_loop();
        } else if self.spins < Self::YIELD_LIMIT {
            std::thread::yield_now();
        } else {
            std::thread::sleep(self.current_sleep);

            let next_sleep = self.current_sleep.as_secs_f32() * Self::SLEEP_MULTIPLIER;

            self.current_sleep = Duration::from_secs_f32(next_sleep).min(self.max_sleep);
        }
        self.spins += 1;
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
