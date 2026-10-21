/// Progress reporting and cancellation interface for long-running PDF operations.
///
/// Operations periodically report `(done, total)` steps and check [`Progress::is_cancelled`]
/// to abort cleanly with [`crate::SheafError::Cancelled`].
pub trait Progress: Send + Sync {
    /// Reports progress where `done` out of `total` units of work have completed.
    fn report(&self, done: u64, total: u64);

    /// Returns `true` if the caller requested cancellation of the running operation.
    fn is_cancelled(&self) -> bool;
}

/// A no-op [`Progress`] implementation for callers that do not need progress updates or cancellation.
///
/// # Examples
///
/// ```
/// use sheaf_core::{NoProgress, Progress};
///
/// let p = NoProgress;
/// p.report(1, 10);
/// assert!(!p.is_cancelled());
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct NoProgress;

impl Progress for NoProgress {
    fn report(&self, _done: u64, _total: u64) {}

    fn is_cancelled(&self) -> bool {
        false
    }
}
