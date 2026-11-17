//! Installation-owned operations usable from the fixed CLI or in its caller's process.

mod config;
mod data_read;
mod driver_digest;
pub mod ffi;
mod service_application;
mod session;
mod setup;
mod supervision;
#[cfg(any(windows, test))]
mod windows_detached;

pub(crate) use session::Outcome;
pub use session::main_entry;
