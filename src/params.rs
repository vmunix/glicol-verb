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

    // === Mappable Knobs (generic, user-assignable in Glicol code) ===
    /// Knob 1 - maps to ~knob1 in Glicol code
    #[id = "knob1"]
    pub knob1: FloatParam,

    /// Knob 2 - maps to ~knob2 in Glicol code
    #[id = "knob2"]
    pub knob2: FloatParam,

    /// Knob 3 - maps to ~knob3 in Glicol code
    #[id = "knob3"]
    pub knob3: FloatParam,

    /// Knob 4 - maps to ~knob4 in Glicol code
    #[id = "knob4"]
    pub knob4: FloatParam,

    // === Effect Parameters (named, for common use cases) ===
    /// Drive amount - maps to ~drive in Glicol code
    #[id = "drive"]
    pub drive: FloatParam,

    /// Feedback amount - maps to ~feedback in Glicol code
    #[id = "feedback"]
    pub feedback: FloatParam,

    /// Effect mix - maps to ~mix in Glicol code
    #[id = "mix"]
    pub mix: FloatParam,

    /// Rate (Hz) - maps to ~rate in Glicol code
    #[id = "rate"]
    pub rate: FloatParam,

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

            // === Mappable Knobs ===
            knob1: FloatParam::new("Knob 1", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

            knob2: FloatParam::new("Knob 2", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

            knob3: FloatParam::new("Knob 3", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

            knob4: FloatParam::new("Knob 4", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

            // === Effect Parameters ===
            drive: FloatParam::new(
                "Drive",
                1.0, // No overdrive by default
                FloatRange::Skewed {
                    min: 1.0,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-1.0), // More resolution at low end
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            feedback: FloatParam::new(
                "Feedback",
                0.3,
                FloatRange::Linear {
                    min: 0.0,
                    max: 0.95,
                }, // Cap at 0.95 to prevent runaway
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            mix: FloatParam::new("Mix", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            rate: FloatParam::new(
                "Rate",
                1.0, // 1 Hz default
                FloatRange::Skewed {
                    min: 0.1,
                    max: 20.0,
                    factor: FloatRange::skew_factor(-1.5), // More resolution at low end
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            code: Arc::new(RwLock::new(
                "out: ~input >> plate 0.5".to_string(), // Plate reverb, 50% wet
            )),
        }
    }
}
