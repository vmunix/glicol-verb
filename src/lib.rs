use crossbeam_channel::{bounded, Receiver, Sender};
use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::Arc;

mod editor;
mod engine;
mod messages;
mod params;

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
            code_receiver,
            code_sender: Some(code_sender),
            user_code: "out: ~input >> plate 0.5".to_string(),
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
                        self.user_code = new_code;
                    }
                    // On error, old code keeps running
                }
            }
        }

        // Collect input samples and dry signal for mixing
        let num_samples = buffer.samples();
        let num_channels = buffer.channels();

        // Ensure we don't exceed our pre-allocated buffer
        let num_samples = num_samples.min(MAX_BUFFER_SIZE);

        // Step 1: Push all input samples to the buffer bridge
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
            self.dry_buffer[i] = input_with_gain;

            self.buffer_bridge.push_input(input_with_gain);
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
            let dry = self.dry_buffer[i];

            wet_max = wet_max.max(wet_left.abs()).max(wet_right.abs());

            // Mix dry/wet and apply output gain
            let out_left = (dry * (1.0 - dry_wet) + wet_left * dry_wet) * output_gain;
            let out_right = (dry * (1.0 - dry_wet) + wet_right * dry_wet) * output_gain;

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

impl ClapPlugin for GlicolVerb {
    const CLAP_ID: &'static str = "com.glicolverb.glicolverb";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Live coding guitar pedal using Glicol");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Distortion,
        ClapFeature::Filter,
        ClapFeature::Delay,
    ];
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

nih_export_clap!(GlicolVerb);
nih_export_vst3!(GlicolVerb);
