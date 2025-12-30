use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};

use super::GLICOL_BLOCK_SIZE;

/// Ring buffer capacity - handles up to 512 sample DAW buffers with margin
const RING_BUFFER_SIZE: usize = 2048;

/// Type aliases for ringbuf producer/consumer
pub type RbProducer<T> = ringbuf::HeapProd<T>;
pub type RbConsumer<T> = ringbuf::HeapCons<T>;

/// Bridges variable-size DAW buffers to fixed-size Glicol blocks.
///
/// DAWs send variable buffer sizes (64, 256, 512 samples).
/// Glicol processes fixed 128-sample blocks.
/// This struct accumulates input samples and provides complete blocks.
pub struct BufferBridge {
    // Input: DAW -> Glicol (mono)
    input_producer: RbProducer<f32>,
    input_consumer: RbConsumer<f32>,

    // Output: Glicol -> DAW (stereo)
    output_left_producer: RbProducer<f32>,
    output_left_consumer: RbConsumer<f32>,
    output_right_producer: RbProducer<f32>,
    output_right_consumer: RbConsumer<f32>,

    // Temporary buffer for Glicol processing
    input_block: [f32; GLICOL_BLOCK_SIZE],

    // Underrun tracking
    underrun_count: u32,
    samples_since_underrun_log: u32,
}

impl BufferBridge {
    pub fn new() -> Self {
        let input_rb = HeapRb::<f32>::new(RING_BUFFER_SIZE);
        let (input_prod, input_cons) = input_rb.split();

        let output_left_rb = HeapRb::<f32>::new(RING_BUFFER_SIZE);
        let (out_l_prod, out_l_cons) = output_left_rb.split();

        let output_right_rb = HeapRb::<f32>::new(RING_BUFFER_SIZE);
        let (out_r_prod, out_r_cons) = output_right_rb.split();

        Self {
            input_producer: input_prod,
            input_consumer: input_cons,
            output_left_producer: out_l_prod,
            output_left_consumer: out_l_cons,
            output_right_producer: out_r_prod,
            output_right_consumer: out_r_cons,
            input_block: [0.0; GLICOL_BLOCK_SIZE],
            underrun_count: 0,
            samples_since_underrun_log: 0,
        }
    }

    /// Push a mono input sample from DAW
    #[inline]
    pub fn push_input(&mut self, sample: f32) {
        // Drop samples if buffer full (prevents blocking)
        let _ = self.input_producer.try_push(sample);
    }

    /// Check if we have enough samples for a Glicol block
    #[inline]
    pub fn has_block(&self) -> bool {
        self.input_consumer.occupied_len() >= GLICOL_BLOCK_SIZE
    }

    /// Pop a block of samples for Glicol processing
    /// Returns a slice of exactly GLICOL_BLOCK_SIZE samples
    pub fn pop_input_block(&mut self) -> &[f32] {
        for i in 0..GLICOL_BLOCK_SIZE {
            self.input_block[i] = self.input_consumer.try_pop().unwrap_or(0.0);
        }
        &self.input_block
    }

    /// Push stereo output from Glicol
    pub fn push_output(&mut self, left: &[f32], right: &[f32]) {
        for &sample in left {
            let _ = self.output_left_producer.try_push(sample);
        }
        for &sample in right {
            let _ = self.output_right_producer.try_push(sample);
        }
    }

    /// Push mono output (duplicated to stereo)
    #[allow(dead_code)]
    pub fn push_output_mono(&mut self, samples: &[f32]) {
        for &sample in samples {
            let _ = self.output_left_producer.try_push(sample);
            let _ = self.output_right_producer.try_push(sample);
        }
    }

    /// Pop a stereo sample pair for DAW output
    /// Returns (left, right), or (0.0, 0.0) if buffer is empty (underrun)
    /// Tracks underruns and logs periodically (rate-limited to avoid spam)
    #[inline]
    pub fn pop_output(&mut self) -> (f32, f32) {
        match (
            self.output_left_consumer.try_pop(),
            self.output_right_consumer.try_pop(),
        ) {
            (Some(left), Some(right)) => (left, right),
            _ => {
                // Underrun - one or both channels empty
                self.underrun_count += 1;
                self.samples_since_underrun_log += 1;

                // Log underruns periodically (~once per second at 44.1kHz)
                if self.samples_since_underrun_log >= 44100 {
                    eprintln!(
                        "[GlicolVerb] Buffer underrun: {} total underruns",
                        self.underrun_count
                    );
                    self.samples_since_underrun_log = 0;
                }

                (0.0, 0.0)
            }
        }
    }

    /// Get the total number of underrun samples since last reset
    #[inline]
    #[allow(dead_code)]
    pub fn underrun_count(&self) -> u32 {
        self.underrun_count
    }

    /// Reset underrun counter
    #[allow(dead_code)]
    pub fn reset_underrun_count(&mut self) {
        self.underrun_count = 0;
    }

    /// Check how many output samples are available
    #[inline]
    pub fn output_available(&self) -> usize {
        self.output_left_consumer
            .occupied_len()
            .min(self.output_right_consumer.occupied_len())
    }

    /// Clear all buffers (call on reset)
    pub fn clear(&mut self) {
        // Clear by consuming all samples
        while self.input_consumer.try_pop().is_some() {}
        while self.output_left_consumer.try_pop().is_some() {}
        while self.output_right_consumer.try_pop().is_some() {}
        self.input_block = [0.0; GLICOL_BLOCK_SIZE];
        // Don't reset underrun_count - keep tracking across resets for diagnostics
        self.samples_since_underrun_log = 0;
    }
}

impl Default for BufferBridge {
    fn default() -> Self {
        Self::new()
    }
}
