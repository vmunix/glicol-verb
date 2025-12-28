/// Messages from GUI to Audio thread
#[derive(Debug, Clone)]
pub enum CodeMessage {
    /// Update the Glicol code
    UpdateCode(String),
}

/// Messages from Audio to GUI thread (status updates)
/// TODO: Implement Audio→GUI feedback channel
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum StatusMessage {
    /// Code update was successful
    Success,
    /// Code update failed with error message
    Error(String),
    /// Buffer underrun occurred
    BufferUnderrun,
}
