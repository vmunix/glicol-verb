mod buffer_bridge;
mod wrapper;

pub use buffer_bridge::BufferBridge;
pub use wrapper::GlicolWrapper;

/// Glicol's fixed block size
pub const GLICOL_BLOCK_SIZE: usize = 128;
