//! Logging abstraction layer
//!
//! This module provides logging macros that work with both `log` and `tracing` crates.
//!
//! # Features
//!
//! - `log` (default) - Uses the standard `log` crate
//! - `tracing` - Uses the `tracing` crate for structured logging
//!
//! Choose one feature at compile time. They are mutually exclusive.
//!
//! # Usage
//!
//! ```ignore
//! use gpui_navigator::{trace_log, debug_log, info_log};
//!
//! trace_log!("Entering function");
//! debug_log!("Navigating to route: {}", path);
//! info_log!("Navigation complete");
//! ```

/// Trace-level logging
///
/// Logs detailed information for debugging purposes.
#[macro_export]
macro_rules! trace_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::trace!($($arg)*);
        #[cfg(feature = "log")]
        ::log::trace!($($arg)*);
    };
}

/// Debug-level logging
///
/// Logs information useful for debugging.
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::debug!($($arg)*);
        #[cfg(feature = "log")]
        ::log::debug!($($arg)*);
    };
}

/// Info-level logging
///
/// Logs general informational messages.
#[macro_export]
macro_rules! info_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::info!($($arg)*);
        #[cfg(feature = "log")]
        ::log::info!($($arg)*);
    };
}

/// Warn-level logging
///
/// Logs warning messages.
#[macro_export]
macro_rules! warn_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::warn!($($arg)*);
        #[cfg(feature = "log")]
        ::log::warn!($($arg)*);
    };
}

/// Error-level logging
///
/// Logs error messages.
#[macro_export]
macro_rules! error_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::error!($($arg)*);
        #[cfg(feature = "log")]
        ::log::error!($($arg)*);
    };
}
