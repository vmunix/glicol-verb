use crossbeam_channel::Sender;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui};
use std::sync::Arc;

use crate::messages::CodeMessage;
use crate::params::GlicolVerbParams;

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
        |_, _| {},
        move |egui_ctx, setter, state| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("GlicolVerb");
                    ui.separator();

                    // === CORE MODULE (always expanded by default) ===
                    egui::CollapsingHeader::new("Core")
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            param_slider!(ui, setter, &params.dry_wet, 0.0..=1.0, "Dry/Wet");
                            gain_slider!(ui, setter, &params.input_gain, "Input Gain");
                            gain_slider!(ui, setter, &params.output_gain, "Output Gain");
                            ui.add_space(4.0);
                        });

                    ui.add_space(4.0);

                    // === EQ MODULE ===
                    let eq_header = if params.eq_bypass.value() {
                        "EQ  ○ Bypassed"
                    } else {
                        "EQ  ● Active"
                    };
                    egui::CollapsingHeader::new(eq_header)
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add_space(4.0);

                            // Bypass toggle
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
                            ui.label("Low Shelf");
                            param_slider!(ui, setter, &params.eq_low_freq, 20.0..=500.0, "Freq");
                            param_slider!(ui, setter, &params.eq_low_gain, -12.0..=12.0, "Gain dB");

                            ui.add_space(8.0);
                            ui.label("Mid Peak");
                            param_slider!(ui, setter, &params.eq_mid_freq, 200.0..=8000.0, "Freq");
                            param_slider!(ui, setter, &params.eq_mid_gain, -12.0..=12.0, "Gain dB");
                            param_slider!(ui, setter, &params.eq_mid_q, 0.5..=4.0, "Q");

                            ui.add_space(8.0);
                            ui.label("High Shelf");
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

                    ui.add_space(4.0);

                    // === DELAY MODULE ===
                    let delay_header = if params.delay_bypass.value() {
                        "Delay  ○ Bypassed"
                    } else {
                        "Delay  ● Active"
                    };
                    egui::CollapsingHeader::new(delay_header)
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add_space(4.0);

                            // Bypass toggle
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
                            param_slider!(ui, setter, &params.delay_time, 1.0..=2000.0, "Time ms");
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

                    ui.add_space(4.0);

                    // === CREATIVE ENGINE (Glicol + Knobs) ===
                    egui::CollapsingHeader::new("Creative Engine")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add_space(4.0);

                            ui.label("Mappable Knobs (~knob1-4)");
                            param_slider!(ui, setter, &params.knob1, 0.0..=1.0, "Knob 1");
                            param_slider!(ui, setter, &params.knob2, 0.0..=1.0, "Knob 2");
                            param_slider!(ui, setter, &params.knob3, 0.0..=1.0, "Knob 3");
                            param_slider!(ui, setter, &params.knob4, 0.0..=1.0, "Knob 4");

                            ui.add_space(8.0);
                            ui.label("Effect Parameters");
                            param_slider!(ui, setter, &params.drive, 1.0..=10.0, "Drive (~drive)");
                            param_slider!(
                                ui,
                                setter,
                                &params.feedback,
                                0.0..=0.95,
                                "Feedback (~feedback)"
                            );
                            param_slider!(ui, setter, &params.mix, 0.0..=1.0, "Mix (~mix)");
                            param_slider!(ui, setter, &params.rate, 0.1..=20.0, "Rate Hz (~rate)");
                            ui.add_space(4.0);
                        });

                    ui.add_space(4.0);

                    // === PRESETS ===
                    egui::CollapsingHeader::new("Presets")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add_space(4.0);

                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Pass-through").clicked() {
                                    state.code_buffer = "out: ~input".to_string();
                                    send_code_update_from_buffer(state);
                                }
                                if ui.button("Plate Reverb").clicked() {
                                    state.code_buffer = "out: ~input >> plate 0.5".to_string();
                                    send_code_update_from_buffer(state);
                                }
                                if ui.button("Overdrive").clicked() {
                                    state.code_buffer =
                                        "out: ~input >> mul ~drive >> lpf 4000.0 0.7".to_string();
                                    send_code_update_from_buffer(state);
                                }
                            });

                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Delay + Feedback").clicked() {
                                    state.code_buffer =
                                        "out: ~input >> delayms 250 >> mul ~feedback".to_string();
                                    send_code_update_from_buffer(state);
                                }
                                if ui.button("Tremolo").clicked() {
                                    state.code_buffer =
                                        "out: ~input >> mul ~mod\n~mod: sin ~rate >> mul 0.5 >> add 0.5"
                                            .to_string();
                                    send_code_update_from_buffer(state);
                                }
                                if ui.button("Filter Sweep").clicked() {
                                    state.code_buffer =
                                        "out: ~input >> lpf ~freq 0.7\n~freq: sin ~rate >> mul 2000 >> add 2500"
                                            .to_string();
                                    send_code_update_from_buffer(state);
                                }
                            });

                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Guitar Amp").clicked() {
                                    state.code_buffer =
                                        "out: ~input >> mul ~drive >> lpf 3000.0 0.5 >> mul 0.7"
                                            .to_string();
                                    send_code_update_from_buffer(state);
                                }
                            });

                            ui.add_space(4.0);
                        });

                    ui.add_space(8.0);
                    ui.separator();

                    // === GLICOL CODE EDITOR (collapsed by default) ===
                    egui::CollapsingHeader::new("Glicol Code Editor")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add_space(4.0);

                            // Status and update button
                            ui.horizontal(|ui| {
                                if ui.button("Update (Ctrl+Enter)").clicked() {
                                    send_code_update_from_buffer(state);
                                }

                                ui.separator();

                                // Status display
                                if state.status_is_error {
                                    ui.colored_label(egui::Color32::RED, &state.status_message);
                                } else if !state.status_message.is_empty() {
                                    ui.colored_label(egui::Color32::GREEN, &state.status_message);
                                }
                            });

                            ui.add_space(4.0);

                            // Variable reference
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Variables:");
                                ui.code("~input");
                                ui.code("~knob1-4");
                                ui.code("~drive");
                                ui.code("~feedback");
                                ui.code("~mix");
                                ui.code("~rate");
                            });

                            ui.add_space(4.0);

                            // Code editor
                            let response = ui.add(
                                egui::TextEdit::multiline(&mut state.code_buffer)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(12)
                                    .id(egui::Id::new("code_editor")),
                            );

                            // Request focus when clicked
                            if response.clicked() {
                                response.request_focus();
                            }

                            // Ctrl+Enter to update
                            if response.has_focus() {
                                let modifiers = ui.input(|i| i.modifiers);
                                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if modifiers.ctrl && enter_pressed {
                                    send_code_update_from_buffer(state);
                                }
                            }

                            ui.add_space(4.0);
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
