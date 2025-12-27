use crossbeam_channel::Sender;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};
use std::sync::Arc;

use crate::messages::CodeMessage;
use crate::params::GlicolVerbParams;

/// Create the plugin editor GUI
pub fn create(
    params: Arc<GlicolVerbParams>,
    code_sender: Sender<CodeMessage>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        EditorState {
            code_sender,
            status_message: String::new(),
            status_is_error: false,
        },
        |_, _| {},
        move |egui_ctx, setter, state| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                // Top bar: Update button and status
                ui.horizontal(|ui| {
                    if ui.button("Update (Ctrl+Enter)").clicked() {
                        send_code_update(&params, &state.code_sender, &mut state.status_message, &mut state.status_is_error);
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
                                let mut code = params.code.write();
                                let response = ui.add(
                                    egui::TextEdit::multiline(&mut *code)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(editor_width - 20.0)
                                        .desired_rows(20)
                                        .lock_focus(true),
                                );

                                // Ctrl+Enter to update
                                if response.has_focus() {
                                    let modifiers = ui.input(|i| i.modifiers);
                                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if modifiers.ctrl && enter_pressed {
                                        drop(code); // Release the lock before sending
                                        send_code_update(&params, &state.code_sender, &mut state.status_message, &mut state.status_is_error);
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

                        ui.add_space(20.0);

                        // Help text
                        ui.group(|ui| {
                            ui.label("Quick Reference");
                            ui.label("~out: output chain (required)");
                            ui.label("~input: guitar input");
                            ui.add_space(5.0);
                            ui.label("Example effects:");
                            ui.label("  ~out: ~input >> mul 0.5");
                            ui.label("  ~out: ~input >> tanh");
                            ui.label("  ~out: ~input >> delay 0.2");
                        });
                    });
                });

                // Bottom: Available variables reference
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Available: ~input, ~out");
                });
            });
        },
    )
}

/// Editor state (not persisted)
struct EditorState {
    code_sender: Sender<CodeMessage>,
    status_message: String,
    status_is_error: bool,
}

/// Send code update to the audio thread
fn send_code_update(
    params: &GlicolVerbParams,
    code_sender: &Sender<CodeMessage>,
    status_message: &mut String,
    status_is_error: &mut bool,
) {
    let code = params.code.read().clone();

    // Basic validation
    if code.trim().is_empty() {
        *status_message = "Error: Code cannot be empty".to_string();
        *status_is_error = true;
        return;
    }

    // Send to audio thread
    match code_sender.try_send(CodeMessage::UpdateCode(code)) {
        Ok(_) => {
            *status_message = "Code sent for update...".to_string();
            *status_is_error = false;
        }
        Err(_) => {
            *status_message = "Error: Message queue full".to_string();
            *status_is_error = true;
        }
    }
}
