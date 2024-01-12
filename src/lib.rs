#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_const_for_fn)]
#![warn(missing_docs)]

//! TODO: crate level docs

pub mod address;
pub mod calibration;
pub mod configuration;
pub mod errors;
pub mod measurements;

mod register;

#[cfg(feature = "async")]
mod r#async;
#[cfg(feature = "async")]
pub use r#async::INA219 as AsyncIna219;

#[cfg(feature = "sync")]
mod sync;
#[cfg(feature = "sync")]
pub use sync::INA219 as SyncIna219;

#[cfg(all(test, feature = "sync"))]
mod tests;
