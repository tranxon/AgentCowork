//! IPC server module

pub mod server;
pub mod transport;
pub mod session;

// Re-export SharedState for convenience
pub use server::SharedState;

// Re-export transport submodules for test access
#[cfg(unix)]
pub use transport::unix_transport;
#[cfg(windows)]
pub use transport::windows_transport;
