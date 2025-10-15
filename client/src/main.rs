use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_renet::transport::NetcodeClientPlugin;
use bevy_renet::*;
use client::{
    camera::{
        camera_follow_player, draw_netcode_ghosts, draw_tile_grid, update_entity_positions,
        update_tree_visuals,
    },
    debug_ui::{handle_debug_keybinds, render_debug_ui},
    setup_client,
    systems::{client_update_system, interpolate_entities, update_confirmed_path},
    ClientState,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .add_plugins(RenetClientPlugin)
        .add_plugins(NetcodeClientPlugin)
        .init_resource::<ClientState>()
        .add_systems(Startup, setup_client)
        .add_systems(
            Update,
            (
                handle_debug_keybinds,
                client_update_system,
                interpolate_entities,
                update_entity_positions,
                update_confirmed_path,
                update_tree_visuals,
                draw_netcode_ghosts,
                draw_tile_grid,
                camera_follow_player,
                render_debug_ui,
            ),
        )
        .run();
}
