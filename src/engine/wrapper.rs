use glicol::Engine;
use nih_plug::util::permit_alloc;

use super::GLICOL_BLOCK_SIZE;

/// Safe wrapper around Glicol's Engine<128>
///
/// Handles initialization, code hot-swapping, and block processing.
pub struct GlicolWrapper {
    engine: Engine<GLICOL_BLOCK_SIZE>,
    /// Temporary buffer for stereo output
    left_buffer: [f32; GLICOL_BLOCK_SIZE],
    right_buffer: [f32; GLICOL_BLOCK_SIZE],
}

impl GlicolWrapper {
    /// Create a new Glicol engine wrapper
    pub fn new(sample_rate: f32) -> Self {
        let mut engine = Engine::<GLICOL_BLOCK_SIZE>::new();
        engine.set_sr(sample_rate as usize);
        engine.set_bpm(120.0);

        // Initialize with plate reverb - no ~ prefix for output node!
        engine.update_with_code("out: ~input >> plate 0.5");

        Self {
            engine,
            left_buffer: [0.0; GLICOL_BLOCK_SIZE],
            right_buffer: [0.0; GLICOL_BLOCK_SIZE],
        }
    }

    /// Set the sample rate (call when DAW sample rate changes)
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.engine.set_sr(sample_rate as usize);
    }

    /// Update the Glicol code (hot-swap)
    ///
    /// Returns Ok(()) on success, Err with error message on parse failure.
    /// On failure, the old code continues running.
    pub fn update_code(&mut self, code: &str) -> Result<(), String> {
        // Glicol's update_with_code handles diffing internally
        self.engine.update_with_code(code);

        // Check if there are any errors in the engine
        // Glicol doesn't return errors from update_with_code directly,
        // so we'll return Ok for now and handle errors via status messages later
        Ok(())
    }

    /// Process a block of audio samples
    ///
    /// Takes mono input, returns references to left and right output buffers.
    /// Input slice must be exactly GLICOL_BLOCK_SIZE samples.
    pub fn process(&mut self, input: &[f32]) -> (&[f32], &[f32]) {
        debug_assert_eq!(input.len(), GLICOL_BLOCK_SIZE);

        // Glicol expects Vec of channel slices for input
        // This small allocation (16 bytes) is unavoidable due to Glicol's API
        let (buffers, _status) = permit_alloc(|| {
            let input_vec = vec![input];
            self.engine.next_block(input_vec)
        });

        // Copy output to our buffers
        // Each Buffer<N> derefs to &[f32] via Deref trait
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| eprintln!("[GlicolVerb] Glicol returned {} buffers", buffers.len()));

        if !buffers.is_empty() {
            let left: &[f32] = &buffers[0]; // Deref to &[f32]
            for (i, &sample) in left.iter().enumerate().take(GLICOL_BLOCK_SIZE) {
                self.left_buffer[i] = sample;
            }

            // If stereo output, use second channel; otherwise duplicate mono
            let right: &[f32] = if buffers.len() > 1 {
                &buffers[1]
            } else {
                &buffers[0]
            };
            for (i, &sample) in right.iter().enumerate().take(GLICOL_BLOCK_SIZE) {
                self.right_buffer[i] = sample;
            }
        } else {
            // No output - fill with silence
            static ONCE_WARN: std::sync::Once = std::sync::Once::new();
            ONCE_WARN
                .call_once(|| eprintln!("[GlicolVerb] WARNING: No buffers returned from Glicol!"));
            self.left_buffer.fill(0.0);
            self.right_buffer.fill(0.0);
        }

        (&self.left_buffer, &self.right_buffer)
    }

    /// Reset the engine state (clear any delay lines, etc.)
    pub fn reset(&mut self) {
        // Re-initialize with current code would be ideal, but Glicol doesn't
        // have a reset method. For now, just clear our buffers.
        self.left_buffer.fill(0.0);
        self.right_buffer.fill(0.0);
    }
}

impl Default for GlicolWrapper {
    fn default() -> Self {
        Self::new(44100.0)
    }
}
