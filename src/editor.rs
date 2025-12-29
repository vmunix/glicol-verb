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
}

/// Helper macro to create a standard egui slider for a nih-plug parameter
/// This works around ParamSlider not responding to mouse events on macOS
macro_rules! param_slider {
    ($ui:expr, $setter:expr, $param:expr, $range:expr, $label:expr) => {{
        $ui.horizontal(|ui| {
            ui.label($label);
            ui.add(
                egui::Slider::from_get_set($range, |new_value| match new_value {
                    Some(v) => {
                        let v = v as f32;
                        $setter.begin_set_parameter($param);
                        $setter.set_parameter($param, v);
                        $setter.end_set_parameter($param);
                        v as f64
                    }
                    None => $param.value() as f64,
                })
                .show_value(true),
            );
        });
    }};
}

/// Helper macro for dB gain parameters (need conversion)
macro_rules! gain_slider {
    ($ui:expr, $setter:expr, $param:expr, $label:expr) => {{
        $ui.horizontal(|ui| {
            ui.label($label);
            ui.add(
                egui::Slider::from_get_set(-30.0..=30.0, |new_value| match new_value {
                    Some(db) => {
                        let gain = util::db_to_gain(db as f32);
                        $setter.begin_set_parameter($param);
                        $setter.set_parameter($param, gain);
                        $setter.end_set_parameter($param);
                        db
                    }
                    None => util::gain_to_db($param.value()) as f64,
                })
                .suffix(" dB")
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
            code_buffer: initial_code,
            status_message: String::new(),
            status_is_error: false,
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
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                // Styled header
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading(
                        egui::RichText::new("GlicolVerb")
                            .color(theme::ACCENT)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Live-coding guitar effects")
                            .color(theme::TEXT_DIM)
                            .italics(),
                    );
                });
                ui.add_space(8.0);

                // Two-column layout: Controls (left) | Glicol Editor (right)
                ui.columns(2, |columns| {
                    // === LEFT COLUMN: DSP Controls ===
                    egui::ScrollArea::vertical()
                        .id_salt("left_scroll")
                        .show(&mut columns[0], |ui| {
                            // === CORE MODULE ===
                            styled_section(ui, "Core", None, true, |ui| {
                                ui.add_space(4.0);
                                param_slider!(ui, setter, &params.dry_wet, 0.0..=1.0, "Dry/Wet");
                                gain_slider!(ui, setter, &params.input_gain, "Input Gain");
                                gain_slider!(ui, setter, &params.output_gain, "Output Gain");
                                ui.add_space(4.0);
                            });

                            // === EQ MODULE ===
                            let eq_active = !params.eq_bypass.value();
                            styled_section(ui, "EQ", Some(eq_active), false, |ui| {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    let bypass_text = if params.eq_bypass.value() {
                                        "Enable"
                                    } else {
                                        "Bypass"
                                    };
                                    if ui.button(bypass_text).clicked() {
                                        let current = params.eq_bypass.value();
                                        setter.begin_set_parameter(&params.eq_bypass);
                                        setter.set_parameter(&params.eq_bypass, !current);
                                        setter.end_set_parameter(&params.eq_bypass);
                                    }
                                });

                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Low Shelf").color(theme::TEXT_NORMAL),
                                );
                                param_slider!(ui, setter, &params.eq_low_freq, 20.0..=500.0, "Freq");
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.eq_low_gain,
                                    -12.0..=12.0,
                                    "Gain dB"
                                );

                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Mid Peak").color(theme::TEXT_NORMAL));
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.eq_mid_freq,
                                    200.0..=8000.0,
                                    "Freq"
                                );
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.eq_mid_gain,
                                    -12.0..=12.0,
                                    "Gain dB"
                                );
                                param_slider!(ui, setter, &params.eq_mid_q, 0.5..=4.0, "Q");

                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("High Shelf").color(theme::TEXT_NORMAL),
                                );
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.eq_high_freq,
                                    2000.0..=20000.0,
                                    "Freq"
                                );
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.eq_high_gain,
                                    -12.0..=12.0,
                                    "Gain dB"
                                );
                                ui.add_space(4.0);
                            });

                            // === DELAY MODULE ===
                            let delay_active = !params.delay_bypass.value();
                            styled_section(ui, "Delay", Some(delay_active), false, |ui| {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    let bypass_text = if params.delay_bypass.value() {
                                        "Enable"
                                    } else {
                                        "Bypass"
                                    };
                                    if ui.button(bypass_text).clicked() {
                                        let current = params.delay_bypass.value();
                                        setter.begin_set_parameter(&params.delay_bypass);
                                        setter.set_parameter(&params.delay_bypass, !current);
                                        setter.end_set_parameter(&params.delay_bypass);
                                    }
                                });

                                ui.add_space(8.0);
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.delay_time,
                                    1.0..=2000.0,
                                    "Time ms"
                                );
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.delay_feedback,
                                    0.0..=0.95,
                                    "Feedback"
                                );
                                param_slider!(ui, setter, &params.delay_mix, 0.0..=1.0, "Mix");
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.delay_highcut,
                                    1000.0..=20000.0,
                                    "High Cut"
                                );
                                ui.add_space(4.0);
                            });

                            // === GLICOL PARAMETERS ===
                            styled_section(ui, "Glicol Parameters", None, true, |ui| {
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new("Use these in your Glicol code")
                                        .color(theme::TEXT_DIM)
                                        .small(),
                                );
                                ui.add_space(4.0);

                                param_slider!(
                                    ui,
                                    setter,
                                    &params.knob1,
                                    0.0..=1.0,
                                    "~knob1"
                                );
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.knob2,
                                    0.0..=1.0,
                                    "~knob2"
                                );
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.knob3,
                                    0.0..=1.0,
                                    "~knob3"
                                );
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.knob4,
                                    0.0..=1.0,
                                    "~knob4"
                                );

                                ui.add_space(8.0);
                                param_slider!(ui, setter, &params.drive, 1.0..=10.0, "~drive");
                                param_slider!(
                                    ui,
                                    setter,
                                    &params.feedback,
                                    0.0..=0.95,
                                    "~feedback"
                                );
                                param_slider!(ui, setter, &params.mix, 0.0..=1.0, "~mix");
                                param_slider!(ui, setter, &params.rate, 0.1..=20.0, "~rate");
                                ui.add_space(4.0);
                            });
                        });

                    // === RIGHT COLUMN: Glicol Code Editor (emphasized) ===
                    egui::ScrollArea::vertical()
                        .id_salt("right_scroll")
                        .show(&mut columns[1], |ui| {
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

                                    // Code editor - takes most of the space
                                    let response = ui.add(
                                        egui::TextEdit::multiline(&mut state.code_buffer)
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(f32::INFINITY)
                                            .desired_rows(18)
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
                                        ui.code("~knob1-4");
                                        ui.code("~drive");
                                        ui.code("~feedback");
                                        ui.code("~mix");
                                        ui.code("~rate");
                                    });
                                });

                            ui.add_space(8.0);

                            // === PRESETS (below editor) ===
                            styled_section(ui, "Presets", None, true, |ui| {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("Pass-through").clicked() {
                                        state.code_buffer = "out: ~input".to_string();
                                        send_code_update_from_buffer(state);
                                    }
                                    if ui.button("Plate Reverb").clicked() {
                                        state.code_buffer =
                                            "out: ~input >> plate 0.5".to_string();
                                        send_code_update_from_buffer(state);
                                    }
                                    if ui.button("Overdrive").clicked() {
                                        state.code_buffer =
                                            "out: ~input >> mul ~drive >> lpf 4000.0 0.7"
                                                .to_string();
                                        send_code_update_from_buffer(state);
                                    }
                                    if ui.button("Tremolo").clicked() {
                                        state.code_buffer =
                                            "out: ~input >> mul ~mod\n~mod: sin ~rate >> mul 0.5 >> add 0.5"
                                                .to_string();
                                        send_code_update_from_buffer(state);
                                    }
                                });
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("Delay + FB").clicked() {
                                        state.code_buffer =
                                            "out: ~input >> delayms 250 >> mul ~feedback"
                                                .to_string();
                                        send_code_update_from_buffer(state);
                                    }
                                    if ui.button("Filter Sweep").clicked() {
                                        state.code_buffer =
                                            "out: ~input >> lpf ~freq 0.7\n~freq: sin ~rate >> mul 2000 >> add 2500"
                                                .to_string();
                                        send_code_update_from_buffer(state);
                                    }
                                    if ui.button("Guitar Amp").clicked() {
                                        state.code_buffer =
                                            "out: ~input >> mul ~drive >> lpf 3000.0 0.5 >> mul 0.7"
                                                .to_string();
                                        send_code_update_from_buffer(state);
                                    }
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
    code_buffer: String, // Local copy for editing
    status_message: String,
    status_is_error: bool,
}

/// Send code update to the audio thread
fn send_code_update_from_buffer(state: &mut EditorState) {
    // Basic validation
    if state.code_buffer.trim().is_empty() {
        state.status_message = "Error: Code cannot be empty".to_string();
        state.status_is_error = true;
        return;
    }

    // Send to audio thread
    match state
        .code_sender
        .try_send(CodeMessage::UpdateCode(state.code_buffer.clone()))
    {
        Ok(_) => {
            state.status_message = "Code updated!".to_string();
            state.status_is_error = false;
        }
        Err(_) => {
            state.status_message = "Error: Message queue full".to_string();
            state.status_is_error = true;
        }
    }
}
