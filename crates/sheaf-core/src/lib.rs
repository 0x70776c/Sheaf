//! Core PDF manipulation library for **Sheaf**.
//!
//! `sheaf-core` provides pure, offline PDF operations without any UI dependencies.

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![warn(missing_docs)]

mod error;
pub mod ops;
mod progress;

pub use error::SheafError;
pub use ops::merge::merge;
pub use progress::{NoProgress, Progress};
