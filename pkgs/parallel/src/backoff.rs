use std::time::Duration;

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
