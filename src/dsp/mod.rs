//! DSP Module Framework
//!
//! Provides the trait and utilities for building stereo DSP processing modules.
//! Each module can be bypassed independently and processes stereo audio.

pub mod delay;
pub mod eq;

/// Stereo audio sample
#[derive(Clone, Copy, Default)]
pub struct StereoSample {
    pub left: f32,
    pub right: f32,
}

impl StereoSample {
    pub fn new(left: f32, right: f32) -> Self {
        Self { left, right }
    }

    #[allow(dead_code)]
    pub fn from_mono(value: f32) -> Self {
        Self {
            left: value,
            right: value,
        }
    }

    pub fn mix(&self, other: StereoSample, wet: f32) -> StereoSample {
        let dry = 1.0 - wet;
        StereoSample {
            left: self.left * dry + other.left * wet,
            right: self.right * dry + other.right * wet,
        }
    }
}

/// Trait for stereo DSP processing modules
pub trait DspModule: Send {
    /// Process a single stereo sample
    fn process(&mut self, input: StereoSample) -> StereoSample;

    /// Set the sample rate (called when audio config changes)
    fn set_sample_rate(&mut self, rate: f32);

    /// Reset internal state (called on transport stop, etc.)
    fn reset(&mut self);

    /// Check if module is bypassed
    fn is_bypassed(&self) -> bool;

    /// Set bypass state
    fn set_bypassed(&mut self, bypassed: bool);

    /// Process with automatic bypass handling
    fn process_with_bypass(&mut self, input: StereoSample) -> StereoSample {
        if self.is_bypassed() {
            input
        } else {
            self.process(input)
        }
    }
}

/// Chain of DSP modules processed in series
#[allow(dead_code)]
pub struct ModuleChain {
    modules: Vec<Box<dyn DspModule>>,
}

#[allow(dead_code)]
impl ModuleChain {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    pub fn add(&mut self, module: Box<dyn DspModule>) {
        self.modules.push(module);
    }

    pub fn process(&mut self, input: StereoSample) -> StereoSample {
        let mut sample = input;
        for module in &mut self.modules {
            sample = module.process_with_bypass(sample);
        }
        sample
    }

    pub fn set_sample_rate(&mut self, rate: f32) {
        for module in &mut self.modules {
            module.set_sample_rate(rate);
        }
    }

    pub fn reset(&mut self) {
        for module in &mut self.modules {
            module.reset();
        }
    }
}

#[allow(dead_code)]
impl Default for ModuleChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModule {
        gain: f32,
        bypassed: bool,
    }

    impl DspModule for TestModule {
        fn process(&mut self, input: StereoSample) -> StereoSample {
            StereoSample {
                left: input.left * self.gain,
                right: input.right * self.gain,
            }
        }

        fn set_sample_rate(&mut self, _rate: f32) {}
        fn reset(&mut self) {}
        fn is_bypassed(&self) -> bool {
            self.bypassed
        }
        fn set_bypassed(&mut self, bypassed: bool) {
            self.bypassed = bypassed;
        }
    }

    #[test]
    fn test_module_chain() {
        let mut chain = ModuleChain::new();
        chain.add(Box::new(TestModule {
            gain: 0.5,
            bypassed: false,
        }));
        chain.add(Box::new(TestModule {
            gain: 2.0,
            bypassed: false,
        }));

        let input = StereoSample::new(1.0, 1.0);
        let output = chain.process(input);

        // 1.0 * 0.5 * 2.0 = 1.0
        assert!((output.left - 1.0).abs() < 0.001);
        assert!((output.right - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_bypass() {
        let mut chain = ModuleChain::new();
        chain.add(Box::new(TestModule {
            gain: 0.5,
            bypassed: true,
        }));

        let input = StereoSample::new(1.0, 1.0);
        let output = chain.process(input);

        // Bypassed, so output == input
        assert!((output.left - 1.0).abs() < 0.001);
    }
}
