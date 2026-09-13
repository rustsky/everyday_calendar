//! The platform-shaped edges: the clock, randomness, storage, timers, focus,
//! and file downloads.
//!
//! Keeping these in one place is what lets `edc-core` stay platform-free and
//! compile into the web client, the desktop app, and the sync server. Each
//! build gets exactly one of the two implementations, with the same names.

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
pub use desktop::*;

#[cfg(not(feature = "desktop"))]
mod web;
#[cfg(not(feature = "desktop"))]
pub use web::*;
