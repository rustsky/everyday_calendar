//! Shared data model for the Every Day Calendar.
//!
//! Deliberately free of platform dependencies: the same types are compiled
//! into the wasm client and the sync server, and both run the same merge.

pub mod bits;
pub mod date;
pub mod legacy;
pub mod model;
pub mod prefs;
pub mod stats;

pub use bits::YearBits;
pub use date::Date;
pub use model::{Accent, Doc, GoalId, GoalRecord, Stamp};
pub use prefs::{Prefs, Theme, View};
