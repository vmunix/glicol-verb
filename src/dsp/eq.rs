//! 3-Band Parametric EQ Module
//!
//! Implements low shelf, mid peak (parametric), and high shelf filters
//! using biquad filter topology.

use super::{DspModule, StereoSample};
use std::f32::consts::PI;

/// Biquad filter coefficients
#[derive(Clone, Copy)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Default for BiquadCoeffs {
    fn default() -> Self {
        // Unity gain passthrough
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

/// Stereo biquad filter state
#[derive(Default)]
struct BiquadState {
    // Left channel state
    x1_l: f32,
    x2_l: f32,
    y1_l: f32,
    y2_l: f32,
    // Right channel state
    x1_r: f32,
    x2_r: f32,
    y1_r: f32,
    y2_r: f32,
}

impl BiquadState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn process(&mut self, input: StereoSample, coeffs: &BiquadCoeffs) -> StereoSample {
        // Left channel
        let out_l = coeffs.b0 * input.left + coeffs.b1 * self.x1_l + coeffs.b2 * self.x2_l
            - coeffs.a1 * self.y1_l
            - coeffs.a2 * self.y2_l;

        self.x2_l = self.x1_l;
        self.x1_l = input.left;
        self.y2_l = self.y1_l;
        self.y1_l = out_l;

        // Right channel
        let out_r = coeffs.b0 * input.right + coeffs.b1 * self.x1_r + coeffs.b2 * self.x2_r
            - coeffs.a1 * self.y1_r
            - coeffs.a2 * self.y2_r;

        self.x2_r = self.x1_r;
        self.x1_r = input.right;
        self.y2_r = self.y1_r;
        self.y1_r = out_r;

        StereoSample::new(out_l, out_r)
    }
}

/// Calculate low shelf filter coefficients
fn calc_low_shelf(freq: f32, gain_db: f32, sample_rate: f32) -> BiquadCoeffs {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / 0.9 - 1.0) + 2.0).sqrt();

    let a_plus_1 = a + 1.0;
    let a_minus_1 = a - 1.0;
    let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

    let a0 = a_plus_1 + a_minus_1 * cos_w0 + two_sqrt_a_alpha;

    BiquadCoeffs {
        b0: (a * (a_plus_1 - a_minus_1 * cos_w0 + two_sqrt_a_alpha)) / a0,
        b1: (2.0 * a * (a_minus_1 - a_plus_1 * cos_w0)) / a0,
        b2: (a * (a_plus_1 - a_minus_1 * cos_w0 - two_sqrt_a_alpha)) / a0,
        a1: (-2.0 * (a_minus_1 + a_plus_1 * cos_w0)) / a0,
        a2: (a_plus_1 + a_minus_1 * cos_w0 - two_sqrt_a_alpha) / a0,
    }
}

/// Calculate high shelf filter coefficients
fn calc_high_shelf(freq: f32, gain_db: f32, sample_rate: f32) -> BiquadCoeffs {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / 0.9 - 1.0) + 2.0).sqrt();

    let a_plus_1 = a + 1.0;
    let a_minus_1 = a - 1.0;
    let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

    let a0 = a_plus_1 - a_minus_1 * cos_w0 + two_sqrt_a_alpha;

    BiquadCoeffs {
        b0: (a * (a_plus_1 + a_minus_1 * cos_w0 + two_sqrt_a_alpha)) / a0,
        b1: (-2.0 * a * (a_minus_1 + a_plus_1 * cos_w0)) / a0,
        b2: (a * (a_plus_1 + a_minus_1 * cos_w0 - two_sqrt_a_alpha)) / a0,
        a1: (2.0 * (a_minus_1 - a_plus_1 * cos_w0)) / a0,
        a2: (a_plus_1 - a_minus_1 * cos_w0 - two_sqrt_a_alpha) / a0,
    }
}

/// Calculate peak (parametric) filter coefficients
fn calc_peak(freq: f32, gain_db: f32, q: f32, sample_rate: f32) -> BiquadCoeffs {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let a0 = 1.0 + alpha / a;

    BiquadCoeffs {
        b0: (1.0 + alpha * a) / a0,
        b1: (-2.0 * cos_w0) / a0,
        b2: (1.0 - alpha * a) / a0,
        a1: (-2.0 * cos_w0) / a0,
        a2: (1.0 - alpha / a) / a0,
    }
}

/// 3-Band EQ: Low Shelf + Mid Peak + High Shelf
pub struct Eq {
    // Filter states
    low_state: BiquadState,
    mid_state: BiquadState,
    high_state: BiquadState,

    // Coefficients
    low_coeffs: BiquadCoeffs,
    mid_coeffs: BiquadCoeffs,
    high_coeffs: BiquadCoeffs,

    // Parameters
    low_freq: f32,
    low_gain: f32,
    mid_freq: f32,
    mid_gain: f32,
    mid_q: f32,
    high_freq: f32,
    high_gain: f32,

    // State
    sample_rate: f32,
    bypassed: bool,
    coeffs_dirty: bool,
}

impl Eq {
    pub fn new(sample_rate: f32) -> Self {
        let mut eq = Self {
            low_state: BiquadState::default(),
            mid_state: BiquadState::default(),
            high_state: BiquadState::default(),
            low_coeffs: BiquadCoeffs::default(),
            mid_coeffs: BiquadCoeffs::default(),
            high_coeffs: BiquadCoeffs::default(),
            low_freq: 200.0,
            low_gain: 0.0,
            mid_freq: 1000.0,
            mid_gain: 0.0,
            mid_q: 1.0,
            high_freq: 4000.0,
            high_gain: 0.0,
            sample_rate,
            bypassed: false,
            coeffs_dirty: true,
        };
        eq.update_coefficients();
        eq
    }

    /// Set low shelf frequency (20-500 Hz)
    pub fn set_low_freq(&mut self, freq: f32) {
        let freq = freq.clamp(20.0, 500.0);
        if (self.low_freq - freq).abs() > 0.01 {
            self.low_freq = freq;
            self.coeffs_dirty = true;
        }
    }

    /// Set low shelf gain (-12 to +12 dB)
    pub fn set_low_gain(&mut self, gain_db: f32) {
        let gain_db = gain_db.clamp(-12.0, 12.0);
        if (self.low_gain - gain_db).abs() > 0.01 {
            self.low_gain = gain_db;
            self.coeffs_dirty = true;
        }
    }

    /// Set mid peak frequency (200-8000 Hz)
    pub fn set_mid_freq(&mut self, freq: f32) {
        let freq = freq.clamp(200.0, 8000.0);
        if (self.mid_freq - freq).abs() > 0.01 {
            self.mid_freq = freq;
            self.coeffs_dirty = true;
        }
    }

    /// Set mid peak gain (-12 to +12 dB)
    pub fn set_mid_gain(&mut self, gain_db: f32) {
        let gain_db = gain_db.clamp(-12.0, 12.0);
        if (self.mid_gain - gain_db).abs() > 0.01 {
            self.mid_gain = gain_db;
            self.coeffs_dirty = true;
        }
    }

    /// Set mid peak Q (0.5-4.0)
    pub fn set_mid_q(&mut self, q: f32) {
        let q = q.clamp(0.5, 4.0);
        if (self.mid_q - q).abs() > 0.01 {
            self.mid_q = q;
            self.coeffs_dirty = true;
        }
    }

    /// Set high shelf frequency (2000-20000 Hz)
    pub fn set_high_freq(&mut self, freq: f32) {
        let freq = freq.clamp(2000.0, 20000.0);
        if (self.high_freq - freq).abs() > 0.01 {
            self.high_freq = freq;
            self.coeffs_dirty = true;
        }
    }

    /// Set high shelf gain (-12 to +12 dB)
    pub fn set_high_gain(&mut self, gain_db: f32) {
        let gain_db = gain_db.clamp(-12.0, 12.0);
        if (self.high_gain - gain_db).abs() > 0.01 {
            self.high_gain = gain_db;
            self.coeffs_dirty = true;
        }
    }

    /// Recalculate filter coefficients if parameters changed
    fn update_coefficients(&mut self) {
        if !self.coeffs_dirty {
            return;
        }

        self.low_coeffs = calc_low_shelf(self.low_freq, self.low_gain, self.sample_rate);
        self.mid_coeffs = calc_peak(self.mid_freq, self.mid_gain, self.mid_q, self.sample_rate);
        self.high_coeffs = calc_high_shelf(self.high_freq, self.high_gain, self.sample_rate);

        self.coeffs_dirty = false;
    }
}

impl DspModule for Eq {
    fn process(&mut self, input: StereoSample) -> StereoSample {
        // Update coefficients if needed
        self.update_coefficients();

        // Process through all three bands in series
        let after_low = self.low_state.process(input, &self.low_coeffs);
        let after_mid = self.mid_state.process(after_low, &self.mid_coeffs);
        self.high_state.process(after_mid, &self.high_coeffs)
    }

    fn set_sample_rate(&mut self, rate: f32) {
        if (rate - self.sample_rate).abs() > 0.1 {
            self.sample_rate = rate;
            self.coeffs_dirty = true;
            self.reset();
        }
    }

    fn reset(&mut self) {
        self.low_state.reset();
        self.mid_state.reset();
        self.high_state.reset();
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
    fn test_eq_passthrough() {
        let mut eq = Eq::new(44100.0);
        // With 0 dB gain on all bands, should pass through unchanged
        eq.set_low_gain(0.0);
        eq.set_mid_gain(0.0);
        eq.set_high_gain(0.0);

        // Process some samples to stabilize filter
        for _ in 0..1000 {
            eq.process(StereoSample::new(0.5, 0.5));
        }

        let input = StereoSample::new(0.5, 0.5);
        let output = eq.process(input);

        // Should be very close to input
        assert!((output.left - input.left).abs() < 0.01);
        assert!((output.right - input.right).abs() < 0.01);
    }

    #[test]
    fn test_eq_boost() {
        let mut eq = Eq::new(44100.0);
        eq.set_low_gain(6.0);
        eq.set_mid_gain(0.0);
        eq.set_high_gain(0.0);

        // Process a low frequency signal (100 Hz sine approximation)
        // Just verify it doesn't explode
        for _ in 0..10000 {
            let output = eq.process(StereoSample::new(0.5, 0.5));
            assert!(output.left.is_finite());
            assert!(output.right.is_finite());
        }
    }

    #[test]
    fn test_bypass() {
        let mut eq = Eq::new(44100.0);
        eq.set_low_gain(12.0);
        eq.set_mid_gain(12.0);
        eq.set_high_gain(12.0);
        eq.set_bypassed(true);

        let input = StereoSample::new(0.5, 0.5);
        let output = eq.process_with_bypass(input);

        // Bypassed, output should equal input
        assert!((output.left - input.left).abs() < 0.001);
    }
}
