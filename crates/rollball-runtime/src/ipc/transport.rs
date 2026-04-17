//! Transport layer abstraction (Unix Socket / Named Pipe / Local TCP)

use async_trait::async_trait;
use rollball_core::protocol::Frame;

/// Transport trait
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(self: Box<Self>, endpoint: &str) -> Result<(), String>;
    async fn send_frame(self: Box<Self>, frame: Frame) -> Result<(), String>;
    async fn recv_frame(self: Box<Self>) -> Result<Frame, String>;
}

/// Create transport based on endpoint scheme
pub fn create_transport(endpoint: &str) -> Box<dyn Transport> {
    if endpoint.starts_with("unix://") {
        Box::new(UnixSocketTransport)
    } else {
        unimplemented!("Unsupported endpoint: {}", endpoint)
    }
}

/// Unix Socket transport (Linux)
pub struct UnixSocketTransport;

#[async_trait]
impl Transport for UnixSocketTransport {
    async fn connect(self: Box<Self>, _endpoint: &str) -> Result<(), String> {
        unimplemented!()
    }

    async fn send_frame(self: Box<Self>, _frame: Frame) -> Result<(), String> {
        unimplemented!()
    }

    async fn recv_frame(self: Box<Self>) -> Result<Frame, String> {
        unimplemented!()
    }
}
