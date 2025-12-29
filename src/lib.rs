use crossbeam_channel::{bounded, Receiver, Sender};
use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::Arc;

mod dsp;
mod editor;
mod engine;
mod messages;
mod params;

use dsp::delay::Delay;
use dsp::eq::Eq;
use dsp::{DspModule, StereoSample};
use engine::{BufferBridge, GlicolWrapper, ParamInjector};
use messages::CodeMessage;
use params::GlicolVerbParams;

/// Maximum buffer size we expect from DAWs (most use 64-2048)
const MAX_BUFFER_SIZE: usize = 4096;

/// GlicolVerb - Live coding guitar pedal VST
pub struct GlicolVerb {
    params: Arc<GlicolVerbParams>,

    /// Glicol audio engine
    engine: GlicolWrapper,

    /// Buffer bridge for DAW <-> Glicol block size conversion
    buffer_bridge: BufferBridge,

    /// EQ module (pre-Glicol)
    eq: Eq,

    /// Delay module (post-Glicol)
    delay: Delay,

    /// Receiver for code updates from GUI
    code_receiver: Receiver<CodeMessage>,

    /// Sender for code updates (given to GUI)
    code_sender: Option<Sender<CodeMessage>>,

    /// Raw user code (before param injection)
    user_code: String,

    /// Parameter injector for ~knob1, ~drive, etc.
    param_injector: ParamInjector,

    /// Sample rate from DAW
    sample_rate: f32,

    /// Pre-allocated buffer for dry samples (avoids allocation in process())
    dry_buffer: [f32; MAX_BUFFER_SIZE],
}

impl Default for GlicolVerb {
    fn default() -> Self {
        // Bounded channel for code updates (capacity 4 is plenty)
        let (code_sender, code_receiver) = bounded(4);

        Self {
            params: Arc::new(GlicolVerbParams::default()),
            engine: GlicolWrapper::new(44100.0),
            buffer_bridge: BufferBridge::new(),
            eq: Eq::new(44100.0),
            delay: Delay::new(44100.0),
            code_receiver,
            code_sender: Some(code_sender),
            user_code: "out: ~input".to_string(),
            param_injector: ParamInjector::new(),
            sample_rate: 44100.0,
            dry_buffer: [0.0; MAX_BUFFER_SIZE],
        }
    }
}

impl GlicolVerb {
    /// Update param_injector with current parameter values
    fn update_param_injector(&mut self) {
        self.param_injector.knob1 = self.params.knob1.value();
        self.param_injector.knob2 = self.params.knob2.value();
        self.param_injector.knob3 = self.params.knob3.value();
        self.param_injector.knob4 = self.params.knob4.value();
        self.param_injector.drive = self.params.drive.value();
        self.param_injector.feedback = self.params.feedback.value();
        self.param_injector.mix = self.params.mix.value();
        self.param_injector.rate = self.params.rate.value();
    }

    /// Update delay module with current parameter values
    fn update_delay_params(&mut self) {
        self.delay.set_bypassed(self.params.delay_bypass.value());
        self.delay.set_time_ms(self.params.delay_time.value());
        self.delay.set_feedback(self.params.delay_feedback.value());
        self.delay.set_mix(self.params.delay_mix.value());
        self.delay.set_highcut(self.params.delay_highcut.value());
    }

    /// Update EQ module with current parameter values
    fn update_eq_params(&mut self) {
        self.eq.set_bypassed(self.params.eq_bypass.value());
        self.eq.set_low_freq(self.params.eq_low_freq.value());
        self.eq.set_low_gain(self.params.eq_low_gain.value());
        self.eq.set_mid_freq(self.params.eq_mid_freq.value());
        self.eq.set_mid_gain(self.params.eq_mid_gain.value());
        self.eq.set_mid_q(self.params.eq_mid_q.value());
        self.eq.set_high_freq(self.params.eq_high_freq.value());
        self.eq.set_high_gain(self.params.eq_high_gain.value());
    }
}

impl Plugin for GlicolVerb {
    const NAME: &'static str = "GlicolVerb";
    const VENDOR: &'static str = "GlicolVerb";
    const URL: &'static str = "https://github.com/your/glicol-verb";
    const EMAIL: &'static str = "your@email.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        // Mono input, stereo output (typical guitar pedal config)
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        // Stereo input/output as fallback
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        // Take the code sender to give to the editor
        let code_sender = self.code_sender.take()?;
        editor::create(self.params.clone(), code_sender)
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;

        // Configure engine for DAW sample rate
        self.engine.set_sample_rate(buffer_config.sample_rate);

        // Configure DSP modules
        self.eq.set_sample_rate(buffer_config.sample_rate);
        self.update_eq_params();
        self.delay.set_sample_rate(buffer_config.sample_rate);
        self.update_delay_params();

        // Initialize with code from params (for state restoration)
        self.user_code = self.params.code.read().clone();

        // Inject current param values and update engine
        self.update_param_injector();
        let injected_code = self.param_injector.inject(&self.user_code);
        let _ = self.engine.update_code(&injected_code);

        true
    }

    fn reset(&mut self) {
        // Clear buffers on transport stop/start
        self.buffer_bridge.clear();
        self.engine.reset();
        self.eq.reset();
        self.delay.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Check for new code from GUI
        while let Ok(msg) = self.code_receiver.try_recv() {
            match msg {
                CodeMessage::UpdateCode(new_code) => {
                    // Capture current param values for injection
                    self.update_param_injector();

                    // Inject param definitions and try to update the engine
                    let injected_code = self.param_injector.inject(&new_code);
                    if self.engine.update_code(&injected_code).is_ok() {
                        self.user_code = new_code.clone();
                        // Update persisted code for state saving
                        *self.params.code.write() = new_code;
                    }
                    // On error, old code keeps running
                }
            }
        }

        // Update DSP module parameters
        self.update_eq_params();
        self.update_delay_params();

        // Collect input samples and dry signal for mixing
        let num_samples = buffer.samples();
        let num_channels = buffer.channels();

        // Ensure we don't exceed our pre-allocated buffer
        let num_samples = num_samples.min(MAX_BUFFER_SIZE);

        // Step 1: Push all input samples to the buffer bridge (through EQ)
        for i in 0..num_samples {
            let input_gain = self.params.input_gain.smoothed.next();

            // Get mono input (average if stereo)
            let input_sample = if num_channels >= 2 {
                let left = buffer.as_slice()[0][i];
                let right = buffer.as_slice()[1][i];
                (left + right) * 0.5
            } else {
                buffer.as_slice()[0][i]
            };

            let input_with_gain = input_sample * input_gain;

            // Process through EQ (mono expanded to stereo, take left channel)
            let eq_input = StereoSample::new(input_with_gain, input_with_gain);
            let eq_output = self.eq.process_with_bypass(eq_input);
            let eq_mono = eq_output.left; // EQ is stereo-linked, so left == right

            self.dry_buffer[i] = eq_mono;

            self.buffer_bridge.push_input(eq_mono);
        }

        // Step 2: Process all available Glicol blocks
        let mut blocks_processed = 0;
        while self.buffer_bridge.has_block() {
            let input_block = self.buffer_bridge.pop_input_block();
            let (left, right) = self.engine.process(input_block);
            self.buffer_bridge.push_output(left, right);
            blocks_processed += 1;
        }

        // Debug: log every ~1 second (assuming 44100 Hz, ~344 calls at 128 samples)
        static DEBUG_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let count = DEBUG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count.is_multiple_of(344) {
            let input_max = self.dry_buffer[..num_samples]
                .iter()
                .map(|x| x.abs())
                .fold(0.0f32, f32::max);
            let output_avail = self.buffer_bridge.output_available();
            eprintln!(
                "[GlicolVerb] samples={}, blocks={}, input_max={:.4}, output_avail={}, code='{}'",
                num_samples,
                blocks_processed,
                input_max,
                output_avail,
                &self.user_code[..self.user_code.len().min(30)]
            );
        }

        // Step 3: Pop output samples and write to DAW buffer
        let output_slices = buffer.as_slice();
        let mut wet_max: f32 = 0.0;
        let mut out_max: f32 = 0.0;

        #[allow(clippy::needless_range_loop)] // Index needed for multi-slice access
        for i in 0..num_samples {
            let output_gain = self.params.output_gain.smoothed.next();
            let dry_wet = self.params.dry_wet.smoothed.next();

            // Get wet sample from Glicol output (may be 0 if buffer underrun)
            let (wet_left, wet_right) = self.buffer_bridge.pop_output();

            // Process through delay module (post-Glicol)
            let glicol_out = StereoSample::new(wet_left, wet_right);
            let delayed = self.delay.process_with_bypass(glicol_out);

            let dry = self.dry_buffer[i];

            wet_max = wet_max.max(delayed.left.abs()).max(delayed.right.abs());

            // Mix dry/wet and apply output gain
            let out_left = (dry * (1.0 - dry_wet) + delayed.left * dry_wet) * output_gain;
            let out_right = (dry * (1.0 - dry_wet) + delayed.right * dry_wet) * output_gain;

            out_max = out_max.max(out_left.abs()).max(out_right.abs());

            // Write to output
            output_slices[0][i] = out_left;
            if num_channels >= 2 {
                output_slices[1][i] = out_right;
            }
        }

        // Log output levels
        if count % 344 == 1 {
            eprintln!(
                "[GlicolVerb] wet_max={:.4}, out_max={:.4}, dry_wet={:.2}",
                wet_max,
                out_max,
                self.params.dry_wet.value()
            );
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for GlicolVerb {
    const VST3_CLASS_ID: [u8; 16] = *b"GlicolVerb__0001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Distortion,
        Vst3SubCategory::Filter,
        Vst3SubCategory::Delay,
    ];
}

nih_export_vst3!(GlicolVerb);
