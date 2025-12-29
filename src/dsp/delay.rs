//! Stereo Delay Module
//!
//! A simple delay effect with feedback and mix controls.
//! Uses circular buffers for efficient delay line implementation.

use super::{DspModule, StereoSample};

/// Maximum delay time in seconds (determines buffer size)
const MAX_DELAY_SECONDS: f32 = 2.0;

/// One-pole lowpass filter for high-cut on feedback
struct OnePole {
    coeff: f32,
    z1_left: f32,
    z1_right: f32,
}

impl OnePole {
    fn new() -> Self {
        Self {
            coeff: 0.5,
            z1_left: 0.0,
            z1_right: 0.0,
        }
    }

    fn set_cutoff(&mut self, freq: f32, sample_rate: f32) {
        // Simple one-pole coefficient: coeff = exp(-2*pi*freq/sr)
        let normalized = (freq / sample_rate).clamp(0.0, 0.5);
        self.coeff = (-std::f32::consts::TAU * normalized).exp();
    }

    fn process(&mut self, input: StereoSample) -> StereoSample {
        // y[n] = (1-coeff)*x[n] + coeff*y[n-1]
        let gain = 1.0 - self.coeff;
        self.z1_left = gain * input.left + self.coeff * self.z1_left;
        self.z1_right = gain * input.right + self.coeff * self.z1_right;
        StereoSample::new(self.z1_left, self.z1_right)
    }

    fn reset(&mut self) {
        self.z1_left = 0.0;
        self.z1_right = 0.0;
    }
}

/// Stereo delay effect
pub struct Delay {
    // Buffer
    buffer_left: Vec<f32>,
    buffer_right: Vec<f32>,
    write_pos: usize,

    // Parameters
    delay_samples: f32,
    feedback: f32,
    mix: f32,
    highcut_freq: f32,

    // State
    sample_rate: f32,
    bypassed: bool,
    filter: OnePole,
}

impl Delay {
    pub fn new(sample_rate: f32) -> Self {
        let buffer_size = (sample_rate * MAX_DELAY_SECONDS) as usize + 1;
        Self {
            buffer_left: vec![0.0; buffer_size],
            buffer_right: vec![0.0; buffer_size],
            write_pos: 0,
            delay_samples: 0.0,
            feedback: 0.0,
            mix: 0.5,
            highcut_freq: 12000.0,
            sample_rate,
            bypassed: false,
            filter: OnePole::new(),
        }
    }

    /// Set delay time in milliseconds (1-2000)
    pub fn set_time_ms(&mut self, ms: f32) {
        let ms = ms.clamp(1.0, MAX_DELAY_SECONDS * 1000.0);
        self.delay_samples = ms * self.sample_rate / 1000.0;
    }

    /// Set feedback amount (0.0-0.95)
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(0.0, 0.95);
    }

    /// Set wet/dry mix (0.0-1.0)
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Set high-cut filter frequency on feedback path (1000-20000 Hz)
    pub fn set_highcut(&mut self, freq: f32) {
        self.highcut_freq = freq.clamp(1000.0, 20000.0);
        self.filter.set_cutoff(self.highcut_freq, self.sample_rate);
    }

    /// Read from delay line with linear interpolation
    fn read_interpolated(&self, buffer: &[f32], delay_samples: f32) -> f32 {
        let buffer_len = buffer.len();
        let read_pos = self.write_pos as f32 - delay_samples;
        let read_pos = if read_pos < 0.0 {
            read_pos + buffer_len as f32
        } else {
            read_pos
        };

        let index0 = read_pos.floor() as usize % buffer_len;
        let index1 = (index0 + 1) % buffer_len;
        let frac = read_pos.fract();

        buffer[index0] * (1.0 - frac) + buffer[index1] * frac
    }
}

impl DspModule for Delay {
    fn process(&mut self, input: StereoSample) -> StereoSample {
        // Read from delay line
        let delayed = StereoSample::new(
            self.read_interpolated(&self.buffer_left, self.delay_samples),
            self.read_interpolated(&self.buffer_right, self.delay_samples),
        );

        // Apply high-cut filter to feedback
        let filtered = self.filter.process(delayed);

        // Write to delay line: input + filtered feedback
        self.buffer_left[self.write_pos] = input.left + filtered.left * self.feedback;
        self.buffer_right[self.write_pos] = input.right + filtered.right * self.feedback;

        // Advance write position
        self.write_pos = (self.write_pos + 1) % self.buffer_left.len();

        // Mix dry and wet
        input.mix(delayed, self.mix)
    }

    fn set_sample_rate(&mut self, rate: f32) {
        if (rate - self.sample_rate).abs() > 0.1 {
            self.sample_rate = rate;
            let buffer_size = (rate * MAX_DELAY_SECONDS) as usize + 1;
            self.buffer_left.resize(buffer_size, 0.0);
            self.buffer_right.resize(buffer_size, 0.0);
            self.filter.set_cutoff(self.highcut_freq, rate);
            self.reset();
        }
    }

    fn reset(&mut self) {
        self.buffer_left.fill(0.0);
        self.buffer_right.fill(0.0);
        self.write_pos = 0;
        self.filter.reset();
    }

    fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_basic() {
        let mut delay = Delay::new(44100.0);
        delay.set_time_ms(100.0); // 100ms delay
        delay.set_feedback(0.0);
        delay.set_mix(1.0); // 100% wet

        // Feed an impulse
        let impulse = StereoSample::new(1.0, 1.0);
        let _ = delay.process(impulse);

        // Process silence until we should hear the delayed impulse
        let delay_samples = (100.0 * 44100.0 / 1000.0) as usize;
        for _ in 0..delay_samples - 1 {
            let out = delay.process(StereoSample::default());
            assert!(out.left.abs() < 0.01);
        }

        // Now we should get the delayed impulse
        let out = delay.process(StereoSample::default());
        assert!(out.left > 0.9);
    }

    #[test]
    fn test_delay_feedback() {
        let mut delay = Delay::new(44100.0);
        delay.set_time_ms(10.0); // Short delay for quick test
        delay.set_feedback(0.5);
        delay.set_mix(1.0);

        // Feed an impulse
        let _ = delay.process(StereoSample::new(1.0, 1.0));

        // Wait for first echo
        let delay_samples = (10.0 * 44100.0 / 1000.0) as usize;
        for _ in 0..delay_samples - 1 {
            delay.process(StereoSample::default());
        }
        let first_echo = delay.process(StereoSample::default());

        // Wait for second echo
        for _ in 0..delay_samples - 1 {
            delay.process(StereoSample::default());
        }
        let second_echo = delay.process(StereoSample::default());

        // Second echo should be quieter due to feedback < 1
        assert!(second_echo.left < first_echo.left);
    }

    #[test]
    fn test_bypass() {
        let mut delay = Delay::new(44100.0);
        delay.set_time_ms(100.0);
        delay.set_mix(1.0);
        delay.set_bypassed(true);

        let input = StereoSample::new(0.5, 0.5);
        let output = delay.process_with_bypass(input);

        // Bypassed, output should equal input
        assert!((output.left - input.left).abs() < 0.001);
    }
}
