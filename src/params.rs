use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use parking_lot::RwLock;
use std::sync::Arc;

/// Plugin parameters
#[derive(Params)]
pub struct GlicolVerbParams {
    /// Editor state (window size, etc.)
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    /// Dry/Wet mix (0.0 = dry, 1.0 = wet)
    #[id = "dry_wet"]
    pub dry_wet: FloatParam,

    /// Input gain in dB
    #[id = "input_gain"]
    pub input_gain: FloatParam,

    /// Output gain in dB
    #[id = "output_gain"]
    pub output_gain: FloatParam,

    /// Persisted Glicol code (not a DAW automatable parameter)
    #[persist = "glicol-code"]
    pub code: Arc<RwLock<String>>,
}

impl Default for GlicolVerbParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(900, 600),

            dry_wet: FloatParam::new(
                "Dry/Wet",
                1.0, // Full wet by default
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            input_gain: FloatParam::new(
                "Input Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            output_gain: FloatParam::new(
                "Output Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            code: Arc::new(RwLock::new(
                "~out: ~input".to_string()
            )),
        }
    }
}
