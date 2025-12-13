use std::time::Duration;

/// Minimal runtime after which to show a progress bar instead of simple log messages.
pub const PROGRESS_BAR_THRESHOLD: Duration = Duration::from_millis(500);
