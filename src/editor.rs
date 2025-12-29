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
            // Right panel for parameters (fixed width)
            egui::SidePanel::right("params_panel")
                .resizable(false)
                .default_width(300.0)
                .show(egui_ctx, |ui| {
                    ui.heading("Parameters");
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // Core controls
                        ui.group(|ui| {
                            ui.label("Core");
                            param_slider!(ui, setter, &params.dry_wet, 0.0..=1.0, "Dry/Wet");
                            gain_slider!(ui, setter, &params.input_gain, "Input");
                            gain_slider!(ui, setter, &params.output_gain, "Output");
                        });

                        ui.add_space(8.0);

                        // Mappable knobs
                        ui.group(|ui| {
                            ui.label("Knobs (~knob1-4)");
                            param_slider!(ui, setter, &params.knob1, 0.0..=1.0, "Knob 1");
                            param_slider!(ui, setter, &params.knob2, 0.0..=1.0, "Knob 2");
                            param_slider!(ui, setter, &params.knob3, 0.0..=1.0, "Knob 3");
                            param_slider!(ui, setter, &params.knob4, 0.0..=1.0, "Knob 4");
                        });

                        ui.add_space(8.0);

                        // Effect parameters
                        ui.group(|ui| {
                            ui.label("Effects");
                            param_slider!(ui, setter, &params.drive, 1.0..=10.0, "Drive");
                            param_slider!(ui, setter, &params.feedback, 0.0..=0.95, "Feedback");
                            param_slider!(ui, setter, &params.mix, 0.0..=1.0, "Mix");
                            param_slider!(ui, setter, &params.rate, 0.1..=20.0, "Rate Hz");
                        });

                        ui.add_space(8.0);

                        // Preset buttons
                        ui.group(|ui| {
                            ui.label("Presets");
                            if ui.button("Pass-through").clicked() {
                                state.code_buffer = "out: ~input".to_string();
                                send_code_update_from_buffer(state);
                            }
                            if ui.button("Overdrive").clicked() {
                                state.code_buffer =
                                    "out: ~input >> mul ~drive >> lpf 4000.0 0.7".to_string();
                                send_code_update_from_buffer(state);
                            }
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
                    });
                });

            // Bottom panel for variable reference
            egui::TopBottomPanel::bottom("vars_panel")
                .resizable(false)
                .show(egui_ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Variables:");
                        ui.label("~input");
                        ui.label("~knob1-4");
                        ui.label("~drive");
                        ui.label("~feedback");
                        ui.label("~mix");
                        ui.label("~rate");
                        ui.separator();
                        ui.label("Output: out:");
                    });
                });

            // Central panel for code editor (fills remaining space)
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                // Top bar: Update button and status
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

                ui.separator();
                ui.heading("Glicol Code");

                // Code editor fills remaining space
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut state.code_buffer)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(20)
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
