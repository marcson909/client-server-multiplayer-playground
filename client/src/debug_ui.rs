use crate::ClientState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

/// debug UI system - renders overlay with netcode stats
pub fn render_debug_ui(
    mut contexts: EguiContexts,
    mut client_state: ResMut<ClientState>,
    time: Res<Time>,
) {
    if !client_state.show_debug_ui {
        return;
    }

    let ctx = contexts.ctx_mut();

    egui::Window::new("Netcode Debug")
        .default_pos([10.0, 10.0])
        .default_width(350.0)
        .show(ctx, |ui| {
            ui.heading("Client-Side Prediction");
            ui.separator();

            ui.checkbox(
                &mut client_state.client_side_prediction,
                "Enable Prediction",
            )
            .on_hover_text("Apply inputs immediately on client before server confirms");

            ui.checkbox(
                &mut client_state.server_reconciliation,
                "Enable Reconciliation",
            )
            .on_hover_text("Re-apply unconfirmed inputs when server state arrives");

            ui.label(format!(
                "Pending Inputs: {}",
                client_state.pending_inputs.len()
            ));
            ui.label(format!(
                "Input Sequence: {}",
                client_state.input_sequence_number
            ));

            ui.add_space(10.0);

            ui.heading("Entity Interpolation");
            ui.separator();

            ui.checkbox(
                &mut client_state.entity_interpolation,
                "Enable Interpolation",
            );

            ui.horizontal(|ui| {
                ui.label("Delay:");
                ui.add(
                    egui::Slider::new(&mut client_state.interpolation_delay, 0.05..=0.3)
                        .text("s")
                        .suffix(" sec"),
                );
            });
            ui.label(format!(
                "{}ms",
                (client_state.interpolation_delay * 1000.0) as u32
            ));

            let mut total_buffers = 0;
            let mut total_snapshots = 0;
            let my_entity_id = client_state.my_entity_id;

            for (entity_id, entity) in client_state.visible_entities.iter() {
                if Some(*entity_id) != my_entity_id && entity.tree.is_none() {
                    total_buffers += 1;
                    total_snapshots += entity.position_buffer.len();
                }
            }

            ui.label(format!("Active Buffers: {}", total_buffers));
            ui.label(format!("Total Snapshots: {}", total_snapshots));

            ui.add_space(10.0);

            ui.heading("Visualization");
            ui.separator();

            ui.checkbox(
                &mut client_state.show_prediction_ghosts,
                "Show Prediction Ghosts",
            );
            ui.label("Display server position vs predicted position");

            ui.checkbox(
                &mut client_state.show_interpolation_ghosts,
                "Show Interpolation Ghosts",
            );
            ui.label("Display interpolation buffer endpoints");

            ui.add_space(10.0);

            ui.heading("Performance");
            ui.separator();

            ui.label(format!("FPS: {:.0}", 1.0 / time.delta_seconds()));
            ui.label(format!("Time: {:.2}s", time.elapsed_seconds_f64()));

            ui.add_space(10.0);

            // Help text
            ui.label("Press F3 to toggle this window");
            ui.label("Press F4 to toggle ghost visuals");
        });
}

/// Handle debug keybinds
pub fn handle_debug_keybinds(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut client_state: ResMut<ClientState>,
) {
    if keyboard.just_pressed(KeyCode::F3) {
        client_state.show_debug_ui = !client_state.show_debug_ui;
        info!(
            "Debug UI: {}",
            if client_state.show_debug_ui {
                "ON"
            } else {
                "OFF"
            }
        );
    }

    if keyboard.just_pressed(KeyCode::F4) {
        let new_state = !client_state.show_prediction_ghosts;
        client_state.show_prediction_ghosts = new_state;
        client_state.show_interpolation_ghosts = new_state;
        info!("Ghost Visuals: {}", if new_state { "ON" } else { "OFF" });
    }

    if keyboard.just_pressed(KeyCode::F5) {
        client_state.client_side_prediction = !client_state.client_side_prediction;
        info!(
            "Prediction: {}",
            if client_state.client_side_prediction {
                "ON"
            } else {
                "OFF"
            }
        );
    }

    if keyboard.just_pressed(KeyCode::F6) {
        client_state.server_reconciliation = !client_state.server_reconciliation;
        info!(
            "Reconciliation: {}",
            if client_state.server_reconciliation {
                "ON"
            } else {
                "OFF"
            }
        );
    }

    if keyboard.just_pressed(KeyCode::F7) {
        client_state.entity_interpolation = !client_state.entity_interpolation;
        info!(
            "Interpolation: {}",
            if client_state.entity_interpolation {
                "ON"
            } else {
                "OFF"
            }
        );
    }
}
