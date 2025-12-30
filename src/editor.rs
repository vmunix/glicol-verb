use crossbeam_channel::Sender;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui};
use std::sync::Arc;

use crate::messages::CodeMessage;
use crate::params::GlicolVerbParams;

/// Dark hardware theme color palette
mod theme {
    use nih_plug_egui::egui::Color32;

    // Background colors
    pub const BG_DARK: Color32 = Color32::from_rgb(25, 25, 28);
    pub const BG_PANEL: Color32 = Color32::from_rgb(38, 38, 42);
    pub const BG_SECTION: Color32 = Color32::from_rgb(32, 32, 36);

    // Accent colors
    pub const ACCENT: Color32 = Color32::from_rgb(100, 140, 180);
    pub const ACCENT_DIM: Color32 = Color32::from_rgb(70, 100, 130);

    // Knob colors (for future custom knob widget)
    #[allow(dead_code)]
    pub const KNOB_BG: Color32 = Color32::from_rgb(45, 45, 50);
    #[allow(dead_code)]
    pub const KNOB_RING: Color32 = Color32::from_rgb(55, 55, 60);
    #[allow(dead_code)]
    pub const KNOB_INDICATOR: Color32 = Color32::from_rgb(220, 140, 80); // Warm orange

    // Text colors
    pub const TEXT_DIM: Color32 = Color32::from_rgb(130, 130, 135);
    pub const TEXT_NORMAL: Color32 = Color32::from_rgb(180, 180, 185);
    pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(220, 220, 225);

    // Status colors
    pub const STATUS_ACTIVE: Color32 = Color32::from_rgb(80, 180, 120);
    pub const STATUS_BYPASS: Color32 = Color32::from_rgb(120, 120, 125);
    pub const STATUS_ERROR: Color32 = Color32::from_rgb(220, 100, 100);

    // Recipe card colors
    pub const CARD_BG: Color32 = Color32::from_rgb(42, 42, 46);
}

/// A recipe is a complete, educational effect with explanation
struct Recipe {
    icon: &'static str,
    name: &'static str,
    description: &'static str,
    code: &'static str,
    explanation: &'static str,
}

/// All available recipes
const RECIPES: &[Recipe] = &[
    Recipe {
        icon: "🎸",
        name: "Amp Sim",
        description: "Drive + tone shaping",
        code: "out: ~input >> mul ~drive >> lpf 3000.0 0.5",
        explanation: "mul boosts signal (adds grit), lpf cuts harsh highs",
    },
    Recipe {
        icon: "🌊",
        name: "Tremolo",
        description: "Pulsing volume effect",
        code: "out: ~input >> mul ~lfo\n~lfo: sin ~rate >> mul 0.5 >> add 0.5",
        explanation: "sin creates a wave, scaled to 0-1 so volume pulses smoothly",
    },
    Recipe {
        icon: "🎚",
        name: "Filter Sweep",
        description: "Auto-wah effect",
        code: "out: ~input >> lpf ~freq 0.7\n~freq: sin ~rate >> mul 2000.0 >> add 2500.0",
        explanation: "Filter cutoff moves with a sine wave for sweeping wah sound",
    },
    Recipe {
        icon: "🔊",
        name: "Slapback",
        description: "Quick doubling echo",
        code: "out: ~input >> delayms 120.0 >> mul 0.6 >> add ~input",
        explanation: "Short delay + dry signal = doubling effect (try 80-150ms)",
    },
    Recipe {
        icon: "🏛",
        name: "Plate Reverb",
        description: "Classic plate space",
        code: "out: ~input >> plate ~mix",
        explanation: "Built-in plate algorithm. ~mix: 0=dry, 1=full reverb",
    },
    Recipe {
        icon: "📼",
        name: "Lo-Fi",
        description: "Gritty retro tone",
        code: "out: ~input >> mul 3.0 >> lpf 2000.0 0.5",
        explanation: "Overdrive + aggressive filtering = vintage character",
    },
    Recipe {
        icon: "🎵",
        name: "Chorus",
        description: "Shimmering doubled sound",
        code: "out: ~input >> add ~chorus\n~chorus: ~input >> delayms ~mod >> mul 0.5\n~mod: sin ~rate >> mul 10.0 >> add 25.0",
        explanation: "Modulated 15-35ms delay mixed with dry = lush doubling",
    },
    Recipe {
        icon: "〰",
        name: "Vibrato",
        description: "Wobbly pitch modulation",
        code: "out: ~input >> delayms ~mod\n~mod: sin ~rate >> mul 3.0 >> add 5.0",
        explanation: "Delay time modulation creates pitch wobble (no dry mix)",
    },
    Recipe {
        icon: "🔔",
        name: "Ring Mod",
        description: "Robotic/bell-like tones",
        code: "out: ~input >> mul ~carrier\n~carrier: sin 200.0 >> mul 0.5 >> add 0.5",
        explanation: "Multiplying by a carrier frequency creates metallic harmonics",
    },
    Recipe {
        icon: "🌌",
        name: "Ambient Wash",
        description: "Atmospheric swells",
        code: "out: ~input >> plate 0.6 >> delayms 400.0 >> mul 0.7 >> add ~input",
        explanation: "Reverb into delay creates dreamy, evolving textures",
    },
    Recipe {
        icon: "🌀",
        name: "Phaser",
        description: "Swirling phase sweep",
        code: "out: ~dry >> add ~wet\n~dry: ~input >> mul 0.5\n~wet: ~input >> apfmsgain ~mod 0.7 >> mul 0.5\n~mod: sin ~rate >> mul 2.0 >> add 4.0",
        explanation: "Allpass filter with modulated delay creates phase cancellation sweeps",
    },
    // Note: Bit Crusher using `meta` node crashes Glicol 0.13.5 - meta isn't fully implemented for ~input
];

/// A building block is a single node snippet users can insert
struct BuildingBlock {
    snippet: &'static str,
    description: &'static str,
}

/// Building block categories
struct BlockCategory {
    name: &'static str,
    blocks: &'static [BuildingBlock],
}

const BUILDING_BLOCKS: &[BlockCategory] = &[
    BlockCategory {
        name: "Filters",
        blocks: &[
            BuildingBlock {
                snippet: "lpf 1000.0 0.7",
                description: "Low-pass (cuts highs)",
            },
            BuildingBlock {
                snippet: "hpf 200.0 0.7",
                description: "High-pass (cuts lows)",
            },
            BuildingBlock {
                snippet: "onepole 800.0",
                description: "Smooth simple filter",
            },
        ],
    },
    BlockCategory {
        name: "Modulation",
        blocks: &[
            BuildingBlock {
                snippet: "sin ~rate",
                description: "Sine LFO",
            },
            BuildingBlock {
                snippet: "saw ~rate",
                description: "Ramp LFO",
            },
            BuildingBlock {
                snippet: "squ ~rate",
                description: "Square LFO",
            },
        ],
    },
    BlockCategory {
        name: "Time",
        blocks: &[
            BuildingBlock {
                snippet: "delayms 250.0",
                description: "Delay (ms)",
            },
            BuildingBlock {
                snippet: "plate 0.5",
                description: "Plate reverb",
            },
        ],
    },
    BlockCategory {
        name: "Gain",
        blocks: &[
            BuildingBlock {
                snippet: "mul 0.5",
                description: "Multiply/volume",
            },
            BuildingBlock {
                snippet: "add ~input",
                description: "Mix signals",
            },
        ],
    },
];

/// Helper macro to create a standard egui slider for a nih-plug parameter
/// This works around ParamSlider not responding to mouse events on macOS
macro_rules! param_slider {
    ($ui:expr, $setter:expr, $param:expr, $range:expr, $label:expr) => {{
        $ui.horizontal(|ui| {
            ui.add_sized([70.0, 18.0], egui::Label::new($label));
            ui.add(
                egui::Slider::from_get_set($range, |new_value| match new_value {
                    Some(v) => {
                        let v = v as f32;
                        $setter.begin_set_parameter($param);
                        $setter.set_parameter($param, v);
                        $setter.end_set_parameter($param);
                        v as f64
                    }
                    None => $param.modulated_plain_value() as f64,
                })
                .show_value(true),
            );
        });
    }};
}

/// Custom painted knob widget with hardware styling (for future use)
#[allow(dead_code)]
fn knob_widget(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
) -> egui::Response {
    let size = egui::Vec2::splat(50.0);

    ui.vertical(|ui| {
        ui.set_width(size.x + 10.0);

        // Allocate space for the knob
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::drag());

        // Handle drag interaction
        if response.dragged() {
            let delta = response.drag_delta();
            let range_size = range.end() - range.start();
            // Vertical drag: up increases, down decreases
            let sensitivity = range_size / 150.0;
            *value = (*value - delta.y * sensitivity).clamp(*range.start(), *range.end());
        }

        // Draw the knob
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let center = rect.center();
            let radius = rect.width() / 2.0 - 3.0;

            // Outer shadow/ring
            painter.circle_filled(
                center + egui::Vec2::new(1.0, 1.0),
                radius + 2.0,
                egui::Color32::from_rgb(15, 15, 18),
            );

            // Outer ring
            painter.circle_filled(center, radius + 2.0, theme::KNOB_RING);

            // Main knob body - layered for depth effect
            painter.circle_filled(center, radius, theme::KNOB_BG);
            painter.circle_filled(
                center - egui::Vec2::new(1.0, 2.0),
                radius - 2.0,
                egui::Color32::from_rgb(52, 52, 58),
            );
            painter.circle_filled(center, radius - 3.0, egui::Color32::from_rgb(48, 48, 54));

            // Inner circle highlight
            painter.circle_stroke(
                center,
                radius - 6.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 58, 64)),
            );

            // Value indicator line
            let normalized = (*value - range.start()) / (range.end() - range.start());
            // Angle goes from 225° (min) to -45° (max), clockwise
            let angle = std::f32::consts::PI * 1.25 - normalized * std::f32::consts::PI * 1.5;
            let indicator_start =
                center + egui::Vec2::new(angle.cos(), -angle.sin()) * (radius * 0.25);
            let indicator_end =
                center + egui::Vec2::new(angle.cos(), -angle.sin()) * (radius * 0.7);

            // Indicator glow
            painter.line_segment(
                [indicator_start, indicator_end],
                egui::Stroke::new(4.0, egui::Color32::from_rgba_unmultiplied(220, 140, 80, 60)),
            );
            // Main indicator
            painter.line_segment(
                [indicator_start, indicator_end],
                egui::Stroke::new(2.5, theme::KNOB_INDICATOR),
            );
        }

        // Label below knob
        ui.add_space(4.0);
        ui.label(egui::RichText::new(label).color(theme::TEXT_DIM).small());

        response
    })
    .inner
}

/// Styled section with dark panel background
fn styled_section<R>(
    ui: &mut egui::Ui,
    title: &str,
    status: Option<bool>, // Some(true) = active, Some(false) = bypassed, None = no status
    default_open: bool,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
    // Build header text with status indicator
    let header_text = match status {
        Some(true) => format!("{}  ●", title),
        Some(false) => format!("{}  ○", title),
        None => title.to_string(),
    };

    let status_color = match status {
        Some(true) => theme::STATUS_ACTIVE,
        Some(false) => theme::STATUS_BYPASS,
        None => theme::TEXT_BRIGHT,
    };

    egui::Frame::new()
        .fill(theme::BG_SECTION)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10))
        .outer_margin(egui::Margin::symmetric(0, 3))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 50)))
        .show(ui, |ui| {
            egui::CollapsingHeader::new(
                egui::RichText::new(header_text)
                    .color(status_color)
                    .strong(),
            )
            .default_open(default_open)
            .show(ui, content)
        })
        .inner
}

/// Render a compact recipe button with hover tooltip
fn recipe_chip(ui: &mut egui::Ui, recipe: &Recipe, state: &mut EditorState) {
    let button_text = format!("{} {}", recipe.icon, recipe.name);
    let tooltip = format!(
        "{}\n\nCode:\n{}\n\n💡 {}",
        recipe.description, recipe.code, recipe.explanation
    );

    let button = egui::Button::new(
        egui::RichText::new(&button_text)
            .color(theme::TEXT_BRIGHT)
            .size(12.0),
    )
    .fill(theme::CARD_BG)
    .corner_radius(egui::CornerRadius::same(4));

    if ui.add(button).on_hover_text(&tooltip).clicked() {
        state.code_buffer = recipe.code.to_string();
        send_code_update_from_buffer(state);
    }
}

/// Render the building blocks section with categories
fn building_blocks_section(ui: &mut egui::Ui, state: &mut EditorState) {
    for category in BUILDING_BLOCKS {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(category.name)
                .color(theme::ACCENT)
                .small()
                .strong(),
        );
        ui.horizontal_wrapped(|ui| {
            for block in category.blocks {
                let button =
                    egui::Button::new(egui::RichText::new(block.snippet).monospace().size(10.0));
                if ui.add(button).on_hover_text(block.description).clicked() {
                    // Append to code with >> prefix if code isn't empty
                    if state.code_buffer.trim().is_empty() {
                        state.code_buffer = format!("out: ~input >> {}", block.snippet);
                    } else if state.code_buffer.ends_with('\n') || state.code_buffer.ends_with(' ')
                    {
                        state.code_buffer.push_str(&format!(">> {}", block.snippet));
                    } else {
                        state
                            .code_buffer
                            .push_str(&format!(" >> {}", block.snippet));
                    }
                }
            }
        });
    }
}

/// Create the plugin editor GUI
pub fn create(
    params: Arc<GlicolVerbParams>,
    code_sender: Sender<CodeMessage>,
) -> Option<Box<dyn Editor>> {
    // Get initial code from params
    let initial_code = params.code.read().clone();

    create_egui_editor(
        params.editor_state.clone(),
        EditorState {
            code_sender,
            code_buffer: initial_code.clone(),
            last_synced_code: initial_code,
            status_message: String::new(),
            status_is_error: false,
            // Initialize EQ state from params
            eq_low_freq: params.eq_low_freq.modulated_plain_value(),
            eq_low_gain: params.eq_low_gain.modulated_plain_value(),
            eq_mid_freq: params.eq_mid_freq.modulated_plain_value(),
            eq_mid_gain: params.eq_mid_gain.modulated_plain_value(),
            eq_mid_q: params.eq_mid_q.modulated_plain_value(),
            eq_high_freq: params.eq_high_freq.modulated_plain_value(),
            eq_high_gain: params.eq_high_gain.modulated_plain_value(),
        },
        |egui_ctx, _| {
            // Configure dark hardware theme
            let mut visuals = egui::Visuals::dark();

            // Background colors
            visuals.window_fill = theme::BG_DARK;
            visuals.panel_fill = theme::BG_PANEL;
            visuals.faint_bg_color = theme::BG_SECTION;
            visuals.extreme_bg_color = theme::BG_DARK;

            // Widget colors
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(48, 48, 52);
            visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_DIM);
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(42, 42, 46);

            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(58, 58, 64);
            visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_NORMAL);

            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(68, 68, 75);
            visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_BRIGHT);

            visuals.widgets.noninteractive.bg_fill = theme::BG_SECTION;
            visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_DIM);

            // Selection colors
            visuals.selection.bg_fill = theme::ACCENT_DIM;
            visuals.selection.stroke = egui::Stroke::new(1.0, theme::ACCENT);

            // Other visual tweaks
            visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 55));

            egui_ctx.set_visuals(visuals);
        },
        move |egui_ctx, setter, state| {
            // Sync code_buffer with params if state was restored externally
            let current_params_code = params.code.read().clone();
            if current_params_code != state.last_synced_code {
                state.code_buffer = current_params_code.clone();
                state.last_synced_code = current_params_code;
            }

            egui::CentralPanel::default().show(egui_ctx, |ui| {
                // Styled header
                ui.add_space(4.0);
                ui.heading(
                    egui::RichText::new("GlicolVerb")
                        .color(theme::ACCENT)
                        .strong(),
                );
                ui.add_space(8.0);

                // Use columns with specific widths for asymmetric layout
                let available_width = ui.available_width();
                let left_width = 220.0;
                let right_width = available_width - left_width - 40.0; // spacing + right margin

                ui.horizontal(|ui| {
                    // === LEFT: Controls (fixed width) ===
                    ui.vertical(|ui| {
                        ui.set_width(left_width);
                        egui::Frame::new()
                            .fill(theme::BG_SECTION)
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::same(10))
                            .show(ui, |ui| {
                                // === GLICOL PARAMETERS ===
                                ui.label(
                                    egui::RichText::new("GLICOL").color(theme::ACCENT).strong(),
                                );
                                ui.label(
                                    egui::RichText::new("Use in code")
                                        .color(theme::TEXT_DIM)
                                        .small(),
                                );
                                ui.add_space(4.0);
                                param_slider!(ui, setter, &params.drive, 1.0..=10.0, "~drive");
                                param_slider!(ui, setter, &params.rate, 0.1..=20.0, "~rate");
                                param_slider!(ui, setter, &params.mix, 0.0..=1.0, "~mix");
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.feedback,
                                    0.0..=0.95,
                                    "~feedback"
                                );

                                ui.add_space(12.0);
                                ui.separator();
                                ui.add_space(8.0);

                                // === CORE ===
                                ui.label(
                                    egui::RichText::new("CORE")
                                        .color(theme::TEXT_NORMAL)
                                        .strong(),
                                );
                                ui.add_space(4.0);
                                param_slider!(ui, setter, &params.dry_wet, 0.0..=1.0, "Dry/Wet");

                                ui.add_space(12.0);
                                ui.separator();
                                ui.add_space(8.0);

                                // === EQ ===
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("EQ")
                                            .color(theme::TEXT_NORMAL)
                                            .strong(),
                                    );
                                    let bypass_text = if params.eq_bypass.value() {
                                        "○"
                                    } else {
                                        "●"
                                    };
                                    let bypass_color = if params.eq_bypass.value() {
                                        theme::STATUS_BYPASS
                                    } else {
                                        theme::STATUS_ACTIVE
                                    };
                                    if ui
                                        .add(egui::Button::new(
                                            egui::RichText::new(bypass_text).color(bypass_color),
                                        ))
                                        .on_hover_text("Toggle EQ bypass")
                                        .clicked()
                                    {
                                        let new_val = !params.eq_bypass.value();
                                        setter.begin_set_parameter(&params.eq_bypass);
                                        setter.set_parameter(&params.eq_bypass, new_val);
                                        setter.end_set_parameter(&params.eq_bypass);
                                    }
                                    if ui
                                        .add(egui::Button::new(
                                            egui::RichText::new("Reset")
                                                .color(theme::TEXT_DIM)
                                                .small(),
                                        ))
                                        .on_hover_text("Reset EQ to flat")
                                        .clicked()
                                    {
                                        // Reset EQ state to defaults
                                        state.eq_low_freq = 200.0;
                                        state.eq_low_gain = 0.0;
                                        state.eq_mid_freq = 1000.0;
                                        state.eq_mid_gain = 0.0;
                                        state.eq_mid_q = 1.0;
                                        state.eq_high_freq = 4000.0;
                                        state.eq_high_gain = 0.0;

                                        // Sync to params
                                        setter.begin_set_parameter(&params.eq_low_freq);
                                        setter.set_parameter(&params.eq_low_freq, 200.0);
                                        setter.end_set_parameter(&params.eq_low_freq);

                                        setter.begin_set_parameter(&params.eq_low_gain);
                                        setter.set_parameter(&params.eq_low_gain, 0.0);
                                        setter.end_set_parameter(&params.eq_low_gain);

                                        setter.begin_set_parameter(&params.eq_mid_freq);
                                        setter.set_parameter(&params.eq_mid_freq, 1000.0);
                                        setter.end_set_parameter(&params.eq_mid_freq);

                                        setter.begin_set_parameter(&params.eq_mid_gain);
                                        setter.set_parameter(&params.eq_mid_gain, 0.0);
                                        setter.end_set_parameter(&params.eq_mid_gain);

                                        setter.begin_set_parameter(&params.eq_mid_q);
                                        setter.set_parameter(&params.eq_mid_q, 1.0);
                                        setter.end_set_parameter(&params.eq_mid_q);

                                        setter.begin_set_parameter(&params.eq_high_freq);
                                        setter.set_parameter(&params.eq_high_freq, 4000.0);
                                        setter.end_set_parameter(&params.eq_high_freq);

                                        setter.begin_set_parameter(&params.eq_high_gain);
                                        setter.set_parameter(&params.eq_high_gain, 0.0);
                                        setter.end_set_parameter(&params.eq_high_gain);
                                    }
                                });
                                ui.add_space(4.0);

                                // Helper to format frequency
                                let fmt_freq = |v: f32| -> String {
                                    if v >= 1000.0 {
                                        format!("{:.1}k", v / 1000.0)
                                    } else {
                                        format!("{:.0}", v)
                                    }
                                };

                                // Low shelf
                                ui.label(egui::RichText::new("Low").color(theme::TEXT_DIM).small());
                                ui.horizontal(|ui| {
                                    ui.add_sized([70.0, 18.0], egui::Label::new("Freq"));
                                    let old_val = state.eq_low_freq;
                                    let slider =
                                        egui::Slider::new(&mut state.eq_low_freq, 20.0..=500.0)
                                            .custom_formatter(|v, _| fmt_freq(v as f32));
                                    if ui.add(slider).changed() && state.eq_low_freq != old_val {
                                        setter.begin_set_parameter(&params.eq_low_freq);
                                        setter
                                            .set_parameter(&params.eq_low_freq, state.eq_low_freq);
                                        setter.end_set_parameter(&params.eq_low_freq);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.add_sized([70.0, 18.0], egui::Label::new("Gain"));
                                    let old_val = state.eq_low_gain;
                                    let slider =
                                        egui::Slider::new(&mut state.eq_low_gain, -12.0..=12.0)
                                            .custom_formatter(|v, _| format!("{:+.1}", v));
                                    if ui.add(slider).changed() && state.eq_low_gain != old_val {
                                        setter.begin_set_parameter(&params.eq_low_gain);
                                        setter
                                            .set_parameter(&params.eq_low_gain, state.eq_low_gain);
                                        setter.end_set_parameter(&params.eq_low_gain);
                                    }
                                });

                                ui.add_space(4.0);

                                // Mid peak
                                ui.label(egui::RichText::new("Mid").color(theme::TEXT_DIM).small());
                                ui.horizontal(|ui| {
                                    ui.add_sized([70.0, 18.0], egui::Label::new("Freq"));
                                    let old_val = state.eq_mid_freq;
                                    let slider =
                                        egui::Slider::new(&mut state.eq_mid_freq, 200.0..=8000.0)
                                            .custom_formatter(|v, _| fmt_freq(v as f32));
                                    if ui.add(slider).changed() && state.eq_mid_freq != old_val {
                                        setter.begin_set_parameter(&params.eq_mid_freq);
                                        setter
                                            .set_parameter(&params.eq_mid_freq, state.eq_mid_freq);
                                        setter.end_set_parameter(&params.eq_mid_freq);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.add_sized([70.0, 18.0], egui::Label::new("Gain"));
                                    let old_val = state.eq_mid_gain;
                                    let slider =
                                        egui::Slider::new(&mut state.eq_mid_gain, -12.0..=12.0)
                                            .custom_formatter(|v, _| format!("{:+.1}", v));
                                    if ui.add(slider).changed() && state.eq_mid_gain != old_val {
                                        setter.begin_set_parameter(&params.eq_mid_gain);
                                        setter
                                            .set_parameter(&params.eq_mid_gain, state.eq_mid_gain);
                                        setter.end_set_parameter(&params.eq_mid_gain);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.add_sized([70.0, 18.0], egui::Label::new("Q"));
                                    let old_val = state.eq_mid_q;
                                    let slider = egui::Slider::new(&mut state.eq_mid_q, 0.5..=4.0);
                                    if ui.add(slider).changed() && state.eq_mid_q != old_val {
                                        setter.begin_set_parameter(&params.eq_mid_q);
                                        setter.set_parameter(&params.eq_mid_q, state.eq_mid_q);
                                        setter.end_set_parameter(&params.eq_mid_q);
                                    }
                                });

                                ui.add_space(4.0);

                                // High shelf
                                ui.label(
                                    egui::RichText::new("High").color(theme::TEXT_DIM).small(),
                                );
                                ui.horizontal(|ui| {
                                    ui.add_sized([70.0, 18.0], egui::Label::new("Freq"));
                                    let old_val = state.eq_high_freq;
                                    let slider = egui::Slider::new(
                                        &mut state.eq_high_freq,
                                        2000.0..=20000.0,
                                    )
                                    .custom_formatter(|v, _| fmt_freq(v as f32));
                                    if ui.add(slider).changed() && state.eq_high_freq != old_val {
                                        setter.begin_set_parameter(&params.eq_high_freq);
                                        setter.set_parameter(
                                            &params.eq_high_freq,
                                            state.eq_high_freq,
                                        );
                                        setter.end_set_parameter(&params.eq_high_freq);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.add_sized([70.0, 18.0], egui::Label::new("Gain"));
                                    let old_val = state.eq_high_gain;
                                    let slider =
                                        egui::Slider::new(&mut state.eq_high_gain, -12.0..=12.0)
                                            .custom_formatter(|v, _| format!("{:+.1}", v));
                                    if ui.add(slider).changed() && state.eq_high_gain != old_val {
                                        setter.begin_set_parameter(&params.eq_high_gain);
                                        setter.set_parameter(
                                            &params.eq_high_gain,
                                            state.eq_high_gain,
                                        );
                                        setter.end_set_parameter(&params.eq_high_gain);
                                    }
                                });
                            });
                    });

                    ui.add_space(8.0);

                    // === RIGHT: Code Editor + Effects Lab (takes remaining space) ===
                    ui.vertical(|ui| {
                        ui.set_width(right_width);
                        // Glicol editor panel with accent styling
                        egui::Frame::new()
                            .fill(theme::BG_SECTION)
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::same(12))
                            .stroke(egui::Stroke::new(1.0, theme::ACCENT_DIM))
                            .show(ui, |ui| {
                                // Header
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("Glicol Code")
                                            .color(theme::ACCENT)
                                            .strong()
                                            .size(16.0),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Update").clicked() {
                                                send_code_update_from_buffer(state);
                                            }
                                            if ui
                                                .button("Reset")
                                                .on_hover_text("Reset to passthrough (no effects)")
                                                .clicked()
                                            {
                                                state.code_buffer = "out: ~input".to_string();
                                                send_code_update_from_buffer(state);
                                            }
                                            // Status display
                                            if state.status_is_error {
                                                ui.colored_label(
                                                    theme::STATUS_ERROR,
                                                    &state.status_message,
                                                );
                                            } else if !state.status_message.is_empty() {
                                                ui.colored_label(
                                                    theme::STATUS_ACTIVE,
                                                    &state.status_message,
                                                );
                                            }
                                        },
                                    );
                                });

                                ui.add_space(8.0);

                                // Code editor - compact but functional
                                let response = ui.add(
                                    egui::TextEdit::multiline(&mut state.code_buffer)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(5)
                                        .id(egui::Id::new("code_editor")),
                                );

                                if response.clicked() {
                                    response.request_focus();
                                }

                                // Ctrl+Enter to update
                                if response.has_focus() {
                                    let modifiers = ui.input(|i| i.modifiers);
                                    let enter_pressed =
                                        ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if modifiers.ctrl && enter_pressed {
                                        send_code_update_from_buffer(state);
                                    }
                                }

                                ui.add_space(8.0);

                                // Available variables reference
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        egui::RichText::new("Variables:")
                                            .color(theme::TEXT_DIM)
                                            .small(),
                                    );
                                    ui.code("~input");
                                    ui.code("~drive");
                                    ui.code("~rate");
                                    ui.code("~mix");
                                    ui.code("~feedback");
                                });
                            });

                        ui.add_space(8.0);

                        // === EFFECTS LAB ===
                        styled_section(ui, "Effects Lab", None, true, |ui| {
                            ui.add_space(4.0);

                            // Recipes section - compact chips
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("RECIPES")
                                        .color(theme::TEXT_NORMAL)
                                        .small()
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new("(click to load, hover for details)")
                                        .color(theme::TEXT_DIM)
                                        .small(),
                                );
                            });
                            ui.add_space(4.0);

                            // Recipe chips in a wrapping grid
                            ui.horizontal_wrapped(|ui| {
                                for recipe in RECIPES {
                                    recipe_chip(ui, recipe, state);
                                }
                            });

                            ui.add_space(8.0);

                            // Building blocks (expanded by default)
                            egui::CollapsingHeader::new(
                                egui::RichText::new("BUILDING BLOCKS")
                                    .color(theme::TEXT_NORMAL)
                                    .small()
                                    .strong(),
                            )
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("Click to append to your code")
                                        .color(theme::TEXT_DIM)
                                        .small(),
                                );
                                building_blocks_section(ui, state);
                            });

                            ui.add_space(4.0);
                        });
                    });
                });
            });
        },
    )
}

/// Editor state (not persisted)
struct EditorState {
    code_sender: Sender<CodeMessage>,
    code_buffer: String,      // Local copy for editing
    last_synced_code: String, // Track what we last synced from params
    status_message: String,
    status_is_error: bool,
    // EQ state - stored locally for immediate UI updates
    eq_low_freq: f32,
    eq_low_gain: f32,
    eq_mid_freq: f32,
    eq_mid_gain: f32,
    eq_mid_q: f32,
    eq_high_freq: f32,
    eq_high_gain: f32,
}

/// Validate Glicol code before sending
/// Returns Ok(()) if valid, or Err with a user-friendly message
fn validate_glicol_code(code: &str) -> Result<Option<String>, String> {
    let trimmed = code.trim();

    // Check for empty code
    if trimmed.is_empty() {
        return Err("Code cannot be empty".to_string());
    }

    // Check for output node - must have "out:" (not "~out:")
    // Allow for whitespace variations like "out :" or "out  :"
    let has_output = trimmed.lines().any(|line| {
        let line = line.trim();
        // Must start with "out" followed by optional whitespace and ":"
        // But NOT start with "~out"
        line.starts_with("out") && !line.starts_with("~out") && line.contains(':')
    });

    if !has_output {
        return Err("Missing 'out:' - code must define an output node (not ~out:)".to_string());
    }

    // Warning (not error) if ~input is not referenced
    let has_input = trimmed.contains("~input");
    if !has_input {
        return Ok(Some(
            "Note: Code doesn't use ~input (live audio)".to_string(),
        ));
    }

    Ok(None)
}

/// Send code update to the audio thread
fn send_code_update_from_buffer(state: &mut EditorState) {
    // Validate the code
    match validate_glicol_code(&state.code_buffer) {
        Err(error) => {
            state.status_message = format!("Error: {}", error);
            state.status_is_error = true;
        }
        Ok(warning) => {
            // Send to audio thread
            match state
                .code_sender
                .try_send(CodeMessage::UpdateCode(state.code_buffer.clone()))
            {
                Ok(_) => {
                    if let Some(warn_msg) = warning {
                        state.status_message = warn_msg;
                        state.status_is_error = false; // Warning, not error
                    } else {
                        state.status_message = "Code updated!".to_string();
                        state.status_is_error = false;
                    }
                }
                Err(_) => {
                    state.status_message = "Error: Message queue full".to_string();
                    state.status_is_error = true;
                }
            }
        }
    }
}
