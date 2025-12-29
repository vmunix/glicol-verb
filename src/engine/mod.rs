mod buffer_bridge;
mod param_injector;
mod wrapper;

pub use buffer_bridge::BufferBridge;
pub use param_injector::ParamInjector;
pub use wrapper::GlicolWrapper;

/// Glicol's fixed block size
pub const GLICOL_BLOCK_SIZE: usize = 128;
