use std::time::Duration;

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
