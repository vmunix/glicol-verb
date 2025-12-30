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

    // === Delay Module Parameters ===
    /// Delay bypass
    #[id = "delay_bypass"]
    pub delay_bypass: BoolParam,

    /// Delay time in milliseconds
    #[id = "delay_time"]
    pub delay_time: FloatParam,

    /// Delay feedback amount
    #[id = "delay_feedback"]
    pub delay_feedback: FloatParam,

    /// Delay wet/dry mix
    #[id = "delay_mix"]
    pub delay_mix: FloatParam,

    /// Delay high-cut filter frequency
    #[id = "delay_highcut"]
    pub delay_highcut: FloatParam,

    // === EQ Module Parameters ===
    /// EQ bypass
    #[id = "eq_bypass"]
    pub eq_bypass: BoolParam,

    /// EQ low shelf frequency
    #[id = "eq_low_freq"]
    pub eq_low_freq: FloatParam,

    /// EQ low shelf gain
    #[id = "eq_low_gain"]
    pub eq_low_gain: FloatParam,

    /// EQ mid peak frequency
    #[id = "eq_mid_freq"]
    pub eq_mid_freq: FloatParam,

    /// EQ mid peak gain
    #[id = "eq_mid_gain"]
    pub eq_mid_gain: FloatParam,

    /// EQ mid peak Q
    #[id = "eq_mid_q"]
    pub eq_mid_q: FloatParam,

    /// EQ high shelf frequency
    #[id = "eq_high_freq"]
    pub eq_high_freq: FloatParam,

    /// EQ high shelf gain
    #[id = "eq_high_gain"]
    pub eq_high_gain: FloatParam,

    /// Persisted Glicol code (not a DAW automatable parameter)
    #[persist = "glicol-code"]
    pub code: Arc<RwLock<String>>,
}

impl Default for GlicolVerbParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1000, 500),

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

            // === Delay Module ===
            delay_bypass: BoolParam::new("Delay Bypass", false),

            delay_time: FloatParam::new(
                "Delay Time",
                250.0, // 250ms default
                FloatRange::Skewed {
                    min: 1.0,
                    max: 2000.0,
                    factor: FloatRange::skew_factor(-1.5), // More resolution at short delays
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            delay_feedback: FloatParam::new(
                "Delay Feedback",
                0.3,
                FloatRange::Linear {
                    min: 0.0,
                    max: 0.95,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            delay_mix: FloatParam::new("Delay Mix", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            delay_highcut: FloatParam::new(
                "Delay High-Cut",
                12000.0,
                FloatRange::Skewed {
                    min: 1000.0,
                    max: 20000.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            // === EQ Module ===
            eq_bypass: BoolParam::new("EQ Bypass", false),

            eq_low_freq: FloatParam::new(
                "EQ Low Freq",
                200.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 500.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            eq_low_gain: FloatParam::new(
                "EQ Low Gain",
                0.0,
                FloatRange::SymmetricalSkewed {
                    min: -12.0,
                    max: 12.0,
                    factor: FloatRange::skew_factor(-1.0), // More resolution near 0 dB
                    center: 0.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            eq_mid_freq: FloatParam::new(
                "EQ Mid Freq",
                1000.0,
                FloatRange::Skewed {
                    min: 200.0,
                    max: 8000.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            eq_mid_gain: FloatParam::new(
                "EQ Mid Gain",
                0.0,
                FloatRange::SymmetricalSkewed {
                    min: -12.0,
                    max: 12.0,
                    factor: FloatRange::skew_factor(-1.0), // More resolution near 0 dB
                    center: 0.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            eq_mid_q: FloatParam::new(
                "EQ Mid Q",
                1.0,
                FloatRange::Skewed {
                    min: 0.5,
                    max: 4.0,
                    factor: FloatRange::skew_factor(-0.5),
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            eq_high_freq: FloatParam::new(
                "EQ High Freq",
                4000.0,
                FloatRange::Skewed {
                    min: 2000.0,
                    max: 20000.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            eq_high_gain: FloatParam::new(
                "EQ High Gain",
                0.0,
                FloatRange::SymmetricalSkewed {
                    min: -12.0,
                    max: 12.0,
                    factor: FloatRange::skew_factor(-1.0), // More resolution near 0 dB
                    center: 0.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            code: Arc::new(RwLock::new(
                "out: ~input".to_string(), // Pass-through
            )),
        }
    }
}
