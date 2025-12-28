use crossbeam_channel::Sender;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};
use std::sync::Arc;

use crate::messages::CodeMessage;
use crate::params::GlicolVerbParams;

// Note: params is captured in the closure but we use a local code_buffer for editing

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

                // Main layout: Editor on left, Parameters on right
                ui.horizontal(|ui| {
                    // Left: Code Editor (70% width)
                    let editor_width = ui.available_width() * 0.7;
                    ui.vertical(|ui| {
                        ui.set_width(editor_width);
                        ui.heading("Glicol Code");

                        egui::ScrollArea::vertical()
                            .max_height(ui.available_height() - 40.0)
                            .show(ui, |ui| {
                                let response = ui.add(
                                    egui::TextEdit::multiline(&mut state.code_buffer)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(editor_width - 20.0)
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
                                    let enter_pressed =
                                        ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if modifiers.ctrl && enter_pressed {
                                        send_code_update_from_buffer(state);
                                    }
                                }
                            });
                    });

                    ui.separator();

                    // Right: Parameter Panel (30% width)
                    ui.vertical(|ui| {
                        ui.heading("Parameters");

                        // Core controls
                        ui.group(|ui| {
                            ui.label("Core");
                            ui.add(widgets::ParamSlider::for_param(&params.dry_wet, setter));
                            ui.add(widgets::ParamSlider::for_param(&params.input_gain, setter));
                            ui.add(widgets::ParamSlider::for_param(&params.output_gain, setter));
                        });

                        ui.add_space(10.0);

                        // Preset buttons (workaround for text input issues)
                        ui.group(|ui| {
                            ui.label("Presets");
                            if ui.button("Pass-through").clicked() {
                                state.code_buffer = "out: ~input".to_string();
                                send_code_update_from_buffer(state);
                            }
                            if ui.button("Half volume").clicked() {
                                state.code_buffer = "out: ~input >> mul 0.5".to_string();
                                send_code_update_from_buffer(state);
                            }
                            if ui.button("Sine 440Hz").clicked() {
                                state.code_buffer = "out: sin 440".to_string();
                                send_code_update_from_buffer(state);
                            }
                            if ui.button("Delay").clicked() {
                                state.code_buffer = "out: ~input >> delayms 250".to_string();
                                send_code_update_from_buffer(state);
                            }
                            if ui.button("Guitar Amp").clicked() {
                                state.code_buffer =
                                    "out: ~input >> mul 2.5 >> lpf 3000.0 0.7 >> plate 0.3"
                                        .to_string();
                                send_code_update_from_buffer(state);
                            }
                        });

                        ui.add_space(10.0);

                        // Help text
                        ui.group(|ui| {
                            ui.label("Quick Reference");
                            ui.label("out: output chain");
                            ui.label("~input: audio input");
                        });
                    });
                });

                // Bottom: Available variables reference
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Available: ~input | Output with: out:");
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
