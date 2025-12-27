use crossbeam_channel::{bounded, Receiver, Sender};
use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::Arc;

mod editor;
mod engine;
mod messages;
mod params;

use engine::BufferBridge;
use messages::CodeMessage;
use params::GlicolVerbParams;

/// GlicolVerb - Live coding guitar pedal VST
pub struct GlicolVerb {
    params: Arc<GlicolVerbParams>,

    /// Buffer bridge for DAW <-> Glicol block size conversion
    buffer_bridge: BufferBridge,

    /// Receiver for code updates from GUI
    code_receiver: Receiver<CodeMessage>,

    /// Sender for code updates (given to GUI)
    code_sender: Option<Sender<CodeMessage>>,

    /// Current Glicol code (updated from GUI)
    current_code: String,

    /// Sample rate from DAW
    sample_rate: f32,
}

impl Default for GlicolVerb {
    fn default() -> Self {
        // Bounded channel for code updates (capacity 4 is plenty)
        let (code_sender, code_receiver) = bounded(4);

        Self {
            params: Arc::new(GlicolVerbParams::default()),
            buffer_bridge: BufferBridge::new(),
            code_receiver,
            code_sender: Some(code_sender),
            current_code: "~out: ~input".to_string(),
            sample_rate: 44100.0,
        }
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

        // Initialize with code from params (for state restoration)
        self.current_code = self.params.code.read().clone();

        true
    }

    fn reset(&mut self) {
        // Clear buffers on transport stop/start
        self.buffer_bridge.clear();
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
                    self.current_code = new_code;
                    // TODO: Phase 2 - update Glicol engine here
                }
            }
        }

        // Get gain parameters
        let input_gain = self.params.input_gain.smoothed.next();
        let output_gain = self.params.output_gain.smoothed.next();
        let dry_wet = self.params.dry_wet.smoothed.next();

        // Phase 1: Simple pass-through with gain
        // Phase 2 will add ring buffer -> Glicol processing

        for mut channel_samples in buffer.iter_samples() {
            // Get input (mono - first channel, or average if stereo input)
            let num_channels = channel_samples.len();
            let input_sample = if num_channels >= 2 {
                // Average stereo to mono
                let left = *channel_samples.get_mut(0).unwrap();
                let right = *channel_samples.get_mut(1).unwrap();
                (left + right) * 0.5
            } else {
                *channel_samples.get_mut(0).unwrap()
            };

            // Apply input gain
            let input_with_gain = input_sample * input_gain;

            // Store dry signal for mix
            let dry_sample = input_with_gain;

            // TODO Phase 2: Push to buffer bridge, process through Glicol
            // For now, just pass through
            let wet_sample = input_with_gain;

            // Mix dry/wet and apply output gain
            let output_sample = (dry_sample * (1.0 - dry_wet) + wet_sample * dry_wet) * output_gain;

            // Write to output (stereo)
            if let Some(left) = channel_samples.get_mut(0) {
                *left = output_sample;
            }
            if let Some(right) = channel_samples.get_mut(1) {
                *right = output_sample;
            }
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
